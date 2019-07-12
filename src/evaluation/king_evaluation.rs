use super::{bitboards, EndGameDisplay, Evaluation, MidGameDisplay};

const SHIELDING_PAWN_MISSING_MG: [i16; 4] = [0, -30, -60, -90];
const SHIELDING_PAWN_MISSING_ON_OPEN_FILE: [i16; 4] = [0, -60, -120, -180];

pub struct KingEvaluation {
    shielding_pawns_missing: i16,
    shielding_pawns_missing_on_open_file: i16,
}

impl Evaluation for KingEvaluation {
    fn eval_mg(&self) -> i16 {
        let mut res = 0;
        res += SHIELDING_PAWN_MISSING_MG[self.shielding_pawns_missing as usize];
        res +=
            SHIELDING_PAWN_MISSING_ON_OPEN_FILE[self.shielding_pawns_missing_on_open_file as usize];
        res
    }
    fn eval_eg(&self) -> i16 {
        0
    }
}

impl MidGameDisplay for KingEvaluation {
    fn display_mg(&self) -> String {
        let mut res_str = String::new();
        res_str.push_str("\tKing-MidGame\n");
        res_str.push_str(&format!(
            "\t\tShielding Pawns missing:              {} -> {}\n",
            self.shielding_pawns_missing,
            SHIELDING_PAWN_MISSING_MG[self.shielding_pawns_missing as usize]
        ));
        res_str.push_str(&format!(
            "\t\tShielding Pawns on open file missing: {} -> {}\n",
            self.shielding_pawns_missing_on_open_file,
            SHIELDING_PAWN_MISSING_ON_OPEN_FILE[self.shielding_pawns_missing_on_open_file as usize]
        ));
        res_str.push_str(&format!("\tSum: {}\n", self.eval_mg()));
        res_str
    }
}

impl EndGameDisplay for KingEvaluation {
    fn display_eg(&self) -> String {
        let mut res_str = String::new();
        res_str.push_str("\tKing-EndGame\n");
        res_str.push_str(&format!("\tSum: {}\n", self.eval_eg()));
        res_str
    }
}

pub fn king_eval(
    king: u64,
    my_pawns: u64,
    enemy_pawns: u64,
    is_white: bool,
    full_moves: usize,
) -> KingEvaluation {
    let king_index = king.trailing_zeros() as usize;
    let mut shield = if is_white {
        bitboards::SHIELDING_PAWNS_WHITE[king_index]
    } else {
        bitboards::SHIELDING_PAWNS_BLACK[king_index]
    };
    let mut king_front_span = if is_white {
        bitboards::w_front_span(king)
    } else {
        bitboards::b_front_span(king)
    };
    king_front_span |= bitboards::west_one(king_front_span) | bitboards::east_one(king_front_span);

    let mut shields_missing = 0;
    let mut shields_on_open_missing = 0;
    if full_moves >= 1 {
        while shield != 0u64 {
            let idx = shield.trailing_zeros() as usize;
            //Block out whole file
            let file = bitboards::FILES[idx % 8];
            if my_pawns & shield & file == 0u64 {
                shields_missing += 1;
                if enemy_pawns & file & king_front_span == 0u64 {
                    shields_on_open_missing += 1;
                }
            }
            shield &= !file;
        }
    }
    KingEvaluation {
        shielding_pawns_missing: shields_missing,
        shielding_pawns_missing_on_open_file: shields_on_open_missing,
    }
}
