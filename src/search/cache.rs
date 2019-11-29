use crate::board_representation::game_state::{
    GameMove, GameMoveType, GameState, PieceType, BISHOP, KNIGHT, PAWN, QUEEN, ROOK,
};
use crate::search::{CombinedSearchParameters, SearchInstruction};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;

pub const INVALID_STATIC_EVALUATION: i16 = -32768;
pub const DEFAULT_LOCKS: usize = 1024;
pub const MIN_LOCKS: usize = 1;
pub const MAX_LOCKS: usize = 65536 * 16; // This is really the maximum!!!
                                         // Else we would need to index by upper_index = (hash >> 47 or lower)
                                         // Using a higher number will lead to the cache not being able to be used fully
pub const DEFAULT_HASH_SIZE: usize = 256; //IN MB
pub const MIN_HASH_SIZE: usize = 0; //IN MB
pub const MAX_HASH_SIZE: usize = 131072; //IN MB

pub struct Cache {
    pub entries: usize,
    pub locks: usize,
    pub buckets_per_lock: usize,
    pub full: AtomicUsize,
    pub cache: Vec<RwLock<Vec<CacheBucket>>>,
}

impl Default for Cache {
    fn default() -> Self {
        Cache::with_size(DEFAULT_HASH_SIZE, DEFAULT_LOCKS)
    }
}

impl Cache {
    pub fn with_size(mb_size: usize, locks: usize) -> Self {
        let buckets = 1024 * 1024 * mb_size / 64;
        let buckets_per_lock = buckets / locks;
        let entries = buckets_per_lock * locks * 3;
        let mut cache = Vec::with_capacity(locks);
        for _ in 0..locks {
            cache.push(RwLock::new(vec![CacheBucket::default(); buckets_per_lock]));
        }
        Cache {
            entries,
            locks,
            buckets_per_lock,
            full: AtomicUsize::new(0),
            cache,
        }
    }
    pub fn get_status(&self) -> f64 {
        if self.entries == 0 {
            return 1000.;
        }
        self.full.load(Ordering::Relaxed) as f64 / self.entries as f64 * 1000.
    }
    pub fn clear(&self) {
        for bucket in &self.cache {
            let mut lock = bucket.write().unwrap();
            *lock = vec![CacheBucket::default(); self.buckets_per_lock];
        }
        self.full.store(0, Ordering::Relaxed);
    }

    pub fn age_entry(&self, hash: u64, new_age: u16) {
        let upper_index = (hash >> 44) as usize % self.locks;
        let lock = unsafe { self.cache.get_unchecked(upper_index) };
        unsafe {
            lock.write()
                .unwrap()
                .get_unchecked_mut(hash as usize % self.buckets_per_lock)
                .age_entry(hash, new_age);
        }
    }

    pub fn get(&self, hash: u64) -> CacheBucket {
        let upper_index = (hash >> 44) as usize % self.locks;
        let lock = unsafe { self.cache.get_unchecked(upper_index) };
        unsafe {
            lock.read()
                .unwrap()
                .get_unchecked(hash as usize % self.buckets_per_lock)
                .clone()
        }
    }

    pub fn insert(
        &self,
        p: &CombinedSearchParameters,
        mv: &GameMove,
        score: i16,
        original_alpha: i16,
        root_plies_played: usize,
        static_evaluation: Option<i16>,
    ) {
        if self.entries == 0 {
            return;
        }
        let upper_index = (p.game_state.hash >> 44) as usize % self.locks;
        let index = p.game_state.hash as usize % self.buckets_per_lock;
        //Aquire lock
        let lock = unsafe { self.cache.get_unchecked(upper_index) };
        let mut write = lock.write().unwrap();
        unsafe {
            if write.get_unchecked_mut(index).replace_entry(
                p,
                mv,
                score,
                original_alpha,
                root_plies_played,
                static_evaluation,
            ) {
                self.full
                    .store(self.full.load(Ordering::Relaxed) + 1, Ordering::Relaxed);
            }
        };
    }

    pub fn lookup(
        &self,
        p: &CombinedSearchParameters,
        static_evaluation: &mut Option<i16>,
        tt_move: &mut Option<GameMove>,
        root_plies: usize,
    ) -> SearchInstruction {
        if self.entries == 0 {
            return SearchInstruction::ContinueSearching;
        }
        let ce = self.get(p.game_state.hash).probe(p.game_state.hash);
        if let Some(ce) = ce {
            if ce.depth >= p.depth_left as i8
                && (p.beta - p.alpha <= 1 || p.depth_left <= 0)
                && (!ce.alpha && !ce.beta
                    || ce.beta && ce.score >= p.beta
                    || ce.alpha && ce.score <= p.alpha)
            {
                *tt_move = Some(CacheEntry::u16_to_mv(ce.mv, p.game_state));
                return SearchInstruction::StopSearching(ce.score);
            }
            if ce.static_evaluation != INVALID_STATIC_EVALUATION {
                *static_evaluation = Some(ce.static_evaluation);
            }
            let mv = CacheEntry::u16_to_mv(ce.mv, p.game_state);
            *tt_move = Some(mv);
            if ce.plies_played != root_plies as u16 {
                self.age_entry(p.game_state.hash, root_plies as u16);
            }
        }
        SearchInstruction::ContinueSearching
    }
}

#[repr(align(64))]
#[derive(Copy, Clone)]
pub struct CacheBucket([CacheEntry; 3]);

impl CacheBucket {
    pub fn replace_entry(
        &mut self,
        p: &CombinedSearchParameters,
        mv: &GameMove,
        score: i16,
        original_alpha: i16,
        root_plies_played: usize,
        static_evaluation: Option<i16>,
    ) -> bool {
        let lower_bound = score >= p.beta;
        let upper_bound = score <= original_alpha;
        let pv_node = p.beta - p.alpha > 1;
        let write_entry = |cache_entry: &mut CacheEntry| {
            cache_entry.write(
                p.game_state.hash,
                p.depth_left,
                root_plies_played as u16,
                score,
                static_evaluation,
                pv_node,
                upper_bound,
                lower_bound,
                &mv,
            )
        };
        let renew_entry = |cache_entry: &mut CacheEntry| -> bool {
            if cache_entry.plies_played < root_plies_played as u16
                || cache_entry.get_score() <= p.depth_left as f64 * if pv_node { 1. } else { 0.7 }
            {
                write_entry(cache_entry);
                true
            } else {
                false
            }
        };

        if self.0[0].is_invalid()
            || self.0[0].plies_played < root_plies_played as u16
            || self.0[0].validate_hash(p.game_state.hash)
        {
            let res = self.0[0].is_invalid();
            renew_entry(&mut self.0[0]);
            return res;
        } else if self.0[1].is_invalid()
            || self.0[1].plies_played < root_plies_played as u16
            || self.0[1].validate_hash(p.game_state.hash)
        {
            let res = self.0[1].is_invalid();
            renew_entry(&mut self.0[1]);
            self.0.swap(0, 1);
            return res;
        } else if self.0[2].is_invalid()
            || self.0[2].plies_played < root_plies_played as u16
            || self.0[2].validate_hash(p.game_state.hash)
        {
            let res = self.0[2].is_invalid();
            renew_entry(&mut self.0[2]);
            self.0.swap(0, 2);
            self.0.swap(1, 2);
            return res;
        }
        let mut min_score = self.0[2].get_score();
        let mut min_entry = 2;

        if self.0[1].get_score() < min_score {
            min_score = self.0[1].get_score();
            min_entry = 1;
        }
        if self.0[0].get_score() < min_score {
            min_score = self.0[0].get_score();
            min_entry = 0;
        }
        let new_score = p.depth_left as f64 * if pv_node { 1. } else { 0.7 };
        if new_score >= min_score {
            write_entry(&mut self.0[min_entry]);
        }
        false
    }

    pub fn probe(&self, hash: u64) -> Option<CacheEntry> {
        if hash == 0u64 {
            return None;
        }
        if self.0[0].validate_hash(hash) {
            return Some(self.0[0]);
        } else if self.0[1].validate_hash(hash) {
            return Some(self.0[1]);
        } else if self.0[2].validate_hash(hash) {
            return Some(self.0[2]);
        }
        None
    }

    pub fn age_entry(&mut self, hash: u64, new_age: u16) {
        if self.0[0].validate_hash(hash) {
            self.0[0].plies_played = new_age;
        } else if self.0[1].validate_hash(hash) {
            self.0[1].plies_played = new_age;
        } else if self.0[2].validate_hash(hash) {
            self.0[2].plies_played = new_age;
        }
    }
}
impl Default for CacheBucket {
    fn default() -> Self {
        CacheBucket([CacheEntry::invalid(); 3])
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CacheEntry {
    pub alpha: bool,
    pub beta: bool,
    pub pv_node: bool,
    pub depth: i8,
    pub plies_played: u16,
    pub score: i16,
    pub upper_hash: u32,
    pub lower_hash: u32,
    pub mv: u16,
    pub static_evaluation: i16,
}

impl CacheEntry {
    pub fn get_score(&self) -> f64 {
        self.depth as f64 * if self.pv_node { 1. } else { 0.7 }
    }

    pub fn validate_hash(&self, hash: u64) -> bool {
        self.upper_hash as u64 == (hash >> 32) && self.lower_hash as u64 == (hash & 0xFFFFFFFF)
    }
    //I know this is not idiomatic, but it saves memory...
    pub fn is_invalid(&self) -> bool {
        self.mv == 0u16
    }
    pub fn invalid() -> CacheEntry {
        CacheEntry {
            upper_hash: 0,
            lower_hash: 0,
            depth: 0,
            plies_played: 0,
            score: 0,
            alpha: false,
            beta: false,
            mv: 0,
            static_evaluation: INVALID_STATIC_EVALUATION,
            pv_node: false,
        }
    }
    pub fn write(
        &mut self,
        hash: u64,
        depth: i16,
        plies_played: u16,
        score: i16,
        static_evaluation: Option<i16>,
        pv_node: bool,
        alpha: bool,
        beta: bool,
        mv: &GameMove,
    ) {
        self.upper_hash = (hash >> 32) as u32;
        self.lower_hash = (hash & 0xFFFFFFFF) as u32;
        self.depth = depth as i8;
        self.plies_played = plies_played;
        self.score = score;
        self.alpha = alpha;
        self.beta = beta;
        self.pv_node = pv_node;
        self.mv = CacheEntry::mv_to_u16(mv);
        self.static_evaluation = if static_evaluation.is_some() {
            static_evaluation.unwrap()
        } else {
            INVALID_STATIC_EVALUATION
        };
    }

    #[inline(always)]
    pub fn mv_to_u16(mv: &GameMove) -> u16 {
        let mut res = 0;
        res |= (mv.from as usize) << 10;
        res |= (mv.to as usize) << 4;
        res |= match &mv.move_type {
            GameMoveType::Quiet => 1,
            GameMoveType::Castle => 2,
            GameMoveType::Promotion(a, _) => match a {
                PieceType::Queen => 3,
                PieceType::Rook => 4,
                PieceType::Bishop => 5,
                PieceType::Knight => 6,
                _ => panic!("Invalid promotion!"),
            },
            GameMoveType::Capture(_) => 7,
            GameMoveType::EnPassant => 8,
        };
        res as u16
    }

    #[inline(always)]
    pub fn u16_to_mv(mv: u16, game_state: &GameState) -> GameMove {
        let typ = mv & 15;
        let from = ((mv & 0xFC00) >> 10) as u8;
        let from_board = 1u64 << from;
        let to = ((mv & 0x03F0) >> 4) as u8;
        let to_board = 1u64 << to;
        let color_to_move = game_state.color_to_move;
        let enemy_color = 1 - color_to_move;
        let piece_type = if (game_state.pieces[PAWN][color_to_move] & from_board) != 0u64 {
            PieceType::Pawn
        } else if (game_state.pieces[KNIGHT][color_to_move] & from_board) != 0u64 {
            PieceType::Knight
        } else if (game_state.pieces[BISHOP][color_to_move] & from_board) != 0u64 {
            PieceType::Bishop
        } else if (game_state.pieces[ROOK][color_to_move] & from_board) != 0u64 {
            PieceType::Rook
        } else if (game_state.pieces[QUEEN][color_to_move] & from_board) != 0u64 {
            PieceType::Queen
        } else {
            PieceType::King
        };
        if typ == 1 {
            GameMove {
                from,
                to,
                piece_type,
                move_type: GameMoveType::Quiet,
            }
        } else if typ == 2 {
            debug_assert_eq!(piece_type, PieceType::King);
            GameMove {
                from,
                to,
                piece_type,
                move_type: GameMoveType::Castle,
            }
        } else {
            if typ == 8 {
                return GameMove {
                    from,
                    to,
                    piece_type,
                    move_type: GameMoveType::EnPassant,
                };
            }
            let captured_piece_type = if (game_state.pieces[PAWN][enemy_color] & to_board) != 0u64 {
                PieceType::Pawn
            } else if (game_state.pieces[KNIGHT][enemy_color] & to_board) != 0u64 {
                PieceType::Knight
            } else if (game_state.pieces[BISHOP][enemy_color] & to_board) != 0u64 {
                PieceType::Bishop
            } else if (game_state.pieces[ROOK][enemy_color] & to_board) != 0u64 {
                PieceType::Rook
            } else if (game_state.pieces[QUEEN][enemy_color] & to_board) != 0u64 {
                PieceType::Queen
            } else {
                PieceType::King
            };
            if typ == 3 {
                GameMove {
                    from,
                    to,
                    piece_type,
                    move_type: GameMoveType::Promotion(
                        PieceType::Queen,
                        if captured_piece_type != PieceType::King {
                            Some(captured_piece_type)
                        } else {
                            None
                        },
                    ),
                }
            } else if typ == 4 {
                GameMove {
                    from,
                    to,
                    piece_type,
                    move_type: GameMoveType::Promotion(
                        PieceType::Rook,
                        if captured_piece_type != PieceType::King {
                            Some(captured_piece_type)
                        } else {
                            None
                        },
                    ),
                }
            } else if typ == 5 {
                GameMove {
                    from,
                    to,
                    piece_type,
                    move_type: GameMoveType::Promotion(
                        PieceType::Bishop,
                        if captured_piece_type != PieceType::King {
                            Some(captured_piece_type)
                        } else {
                            None
                        },
                    ),
                }
            } else if typ == 6 {
                GameMove {
                    from,
                    to,
                    piece_type,
                    move_type: GameMoveType::Promotion(
                        PieceType::Knight,
                        if captured_piece_type != PieceType::King {
                            Some(captured_piece_type)
                        } else {
                            None
                        },
                    ),
                }
            } else {
                GameMove {
                    from,
                    to,
                    piece_type,
                    move_type: GameMoveType::Capture(captured_piece_type),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CacheEntry;
    use crate::board_representation::game_state::{GameMove, GameMoveType, GameState, PieceType};
    use crate::move_generation::makemove::make_move;

    #[test]
    fn mv_to_u16_test() {
        let mut game_state = GameState::from_fen("k4b2/2p1P3/8/3P4/6b1/7P/8/R3K2R w KQ -");
        {
            let h3h4 = GameMove {
                from: 23,
                to: 31,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Quiet,
            };
            let h3h4u16 = CacheEntry::mv_to_u16(&h3h4);
            let h3h4res = CacheEntry::u16_to_mv(h3h4u16, &game_state);
            assert_eq!(h3h4res.move_type, h3h4.move_type);
            assert_eq!(h3h4res.piece_type, h3h4.piece_type);
            assert_eq!(h3h4res.from, h3h4.from);
            assert_eq!(h3h4res.to, h3h4.to);
        }
        {
            let h3g4 = GameMove {
                from: 23,
                to: 30,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Capture(PieceType::Bishop),
            };
            let h3g4u16 = CacheEntry::mv_to_u16(&h3g4);
            let h3g4res = CacheEntry::u16_to_mv(h3g4u16, &game_state);
            assert_eq!(h3g4res.from, h3g4.from);
            assert_eq!(h3g4res.to, h3g4.to);
            assert_eq!(h3g4res.move_type, h3g4.move_type);
            assert_eq!(h3g4res.piece_type, h3g4.piece_type);
        }
        {
            let e1c1 = GameMove {
                from: 4,
                to: 2,
                piece_type: PieceType::King,
                move_type: GameMoveType::Castle,
            };
            let e1c1u16 = CacheEntry::mv_to_u16(&e1c1);
            let e1c1res = CacheEntry::u16_to_mv(e1c1u16, &game_state);
            assert_eq!(e1c1res.from, e1c1.from);
            assert_eq!(e1c1res.to, e1c1.to);
            assert_eq!(e1c1res.move_type, e1c1.move_type);
            assert_eq!(e1c1res.piece_type, e1c1.piece_type);
        }
        {
            let e1g1 = GameMove {
                from: 4,
                to: 6,
                piece_type: PieceType::King,
                move_type: GameMoveType::Castle,
            };
            let e1g1u16 = CacheEntry::mv_to_u16(&e1g1);
            let e1g1res = CacheEntry::u16_to_mv(e1g1u16, &game_state);
            assert_eq!(e1g1res.from, e1g1.from);
            assert_eq!(e1g1res.to, e1g1.to);
            assert_eq!(e1g1res.move_type, e1g1.move_type);
            assert_eq!(e1g1res.piece_type, e1g1.piece_type);
        }
        {
            let e7e8q = GameMove {
                from: 52,
                to: 60,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Queen, None),
            };
            let e7e8qu16 = CacheEntry::mv_to_u16(&e7e8q);
            let e7e8qres = CacheEntry::u16_to_mv(e7e8qu16, &game_state);
            assert_eq!(e7e8qres.from, e7e8q.from);
            assert_eq!(e7e8qres.to, e7e8q.to);
            assert_eq!(e7e8qres.move_type, e7e8q.move_type);
            assert_eq!(e7e8qres.piece_type, e7e8q.piece_type);
        }
        {
            let e7e8r = GameMove {
                from: 52,
                to: 60,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Rook, None),
            };
            let e7e8ru16 = CacheEntry::mv_to_u16(&e7e8r);
            let e7e8rres = CacheEntry::u16_to_mv(e7e8ru16, &game_state);
            assert_eq!(e7e8rres.from, e7e8r.from);
            assert_eq!(e7e8rres.to, e7e8r.to);
            assert_eq!(e7e8rres.move_type, e7e8r.move_type);
            assert_eq!(e7e8rres.piece_type, e7e8r.piece_type);
        }
        {
            let e7e8b = GameMove {
                from: 52,
                to: 60,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Bishop, None),
            };
            let e7e8bu16 = CacheEntry::mv_to_u16(&e7e8b);
            let e7e8bres = CacheEntry::u16_to_mv(e7e8bu16, &game_state);
            assert_eq!(e7e8bres.from, e7e8b.from);
            assert_eq!(e7e8bres.to, e7e8b.to);
            assert_eq!(e7e8bres.move_type, e7e8b.move_type);
            assert_eq!(e7e8bres.piece_type, e7e8b.piece_type);
        }
        {
            let e7e8n = GameMove {
                from: 52,
                to: 60,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Knight, None),
            };
            let e7e8nu16 = CacheEntry::mv_to_u16(&e7e8n);
            let e7e8nres = CacheEntry::u16_to_mv(e7e8nu16, &game_state);
            assert_eq!(e7e8nres.from, e7e8n.from);
            assert_eq!(e7e8nres.to, e7e8n.to);
            assert_eq!(e7e8nres.move_type, e7e8n.move_type);
            assert_eq!(e7e8nres.piece_type, e7e8n.piece_type);
        }

        {
            let e7e8q = GameMove {
                from: 52,
                to: 61,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Queen, Some(PieceType::Bishop)),
            };
            let e7e8qu16 = CacheEntry::mv_to_u16(&e7e8q);
            let e7e8qres = CacheEntry::u16_to_mv(e7e8qu16, &game_state);
            assert_eq!(e7e8qres.from, e7e8q.from);
            assert_eq!(e7e8qres.to, e7e8q.to);
            assert_eq!(e7e8qres.move_type, e7e8q.move_type);
            assert_eq!(e7e8qres.piece_type, e7e8q.piece_type);
        }
        {
            let e7e8r = GameMove {
                from: 52,
                to: 61,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Rook, Some(PieceType::Bishop)),
            };
            let e7e8ru16 = CacheEntry::mv_to_u16(&e7e8r);
            let e7e8rres = CacheEntry::u16_to_mv(e7e8ru16, &game_state);
            assert_eq!(e7e8rres.from, e7e8r.from);
            assert_eq!(e7e8rres.to, e7e8r.to);
            assert_eq!(e7e8rres.move_type, e7e8r.move_type);
            assert_eq!(e7e8rres.piece_type, e7e8r.piece_type);
        }
        {
            let e7e8b = GameMove {
                from: 52,
                to: 61,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Bishop, Some(PieceType::Bishop)),
            };
            let e7e8bu16 = CacheEntry::mv_to_u16(&e7e8b);
            let e7e8bres = CacheEntry::u16_to_mv(e7e8bu16, &game_state);
            assert_eq!(e7e8bres.from, e7e8b.from);
            assert_eq!(e7e8bres.to, e7e8b.to);
            assert_eq!(e7e8bres.move_type, e7e8b.move_type);
            assert_eq!(e7e8bres.piece_type, e7e8b.piece_type);
        }
        {
            let e7e8n = GameMove {
                from: 52,
                to: 61,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Promotion(PieceType::Knight, Some(PieceType::Bishop)),
            };
            let e7e8nu16 = CacheEntry::mv_to_u16(&e7e8n);
            let e7e8nres = CacheEntry::u16_to_mv(e7e8nu16, &game_state);
            assert_eq!(e7e8nres.from, e7e8n.from);
            assert_eq!(e7e8nres.to, e7e8n.to);
            assert_eq!(e7e8nres.move_type, e7e8n.move_type);
            assert_eq!(e7e8nres.piece_type, e7e8n.piece_type);
        }
        game_state = make_move(
            &game_state,
            &GameMove {
                from: 23,
                to: 31,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Quiet,
            },
        );
        game_state = make_move(
            &game_state,
            &GameMove {
                from: 50,
                to: 34,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::Quiet,
            },
        );
        {
            let d5d6 = GameMove {
                from: 35,
                to: 42,
                piece_type: PieceType::Pawn,
                move_type: GameMoveType::EnPassant,
            };
            let d5d6u16 = CacheEntry::mv_to_u16(&d5d6);
            let d5d6res = CacheEntry::u16_to_mv(d5d6u16, &game_state);
            assert_eq!(d5d6res.from, d5d6.from);
            assert_eq!(d5d6res.to, d5d6.to);
            assert_eq!(d5d6res.move_type, d5d6.move_type);
            assert_eq!(d5d6res.piece_type, d5d6.piece_type);
        }
    }
}
