use crate::board_representation::game_state::GameState;
use crate::search::cache::{DEFAULT_HASH_SIZE, DEFAULT_LOCKS};
use crate::search::searcher::DEFAULT_THREADS;

pub struct UCIEngine<'a> {
    pub name: &'a str,
    pub author: &'a str,
    pub internal_state: GameState,
    pub hash_size: usize,
    pub hash_locks: usize,
    pub threads: usize,
}

impl<'a> UCIEngine<'a> {
    pub fn standard() -> UCIEngine<'a> {
        UCIEngine {
            name: &"FabChessDev v1.12.7",
            author: &"Fabian von der Warth, Contributor: Erik Imgrund",
            internal_state: GameState::standard(),
            hash_size: DEFAULT_HASH_SIZE,
            hash_locks: DEFAULT_LOCKS,
            threads: DEFAULT_THREADS,
        }
    }

    pub fn id_command(&self) {
        println!("id name {}", self.name);
        println!("id author {}", self.author);
    }
}
