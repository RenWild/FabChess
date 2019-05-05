use super::{bitboards, EndGameDisplay, Evaluation, MidGameDisplay};

pub const PAWN_PIECE_VALUE_MG: i16 = 140;
pub const PAWN_PIECE_VALUE_EG: i16 = 160;
const PAWN_DOUBLED_VALUE_MG: i16 = -8;
const PAWN_DOUBLED_VALUE_EG: i16 = -37;
const PAWN_ISOLATED_VALUE_MG: i16 = -5;
const PAWN_ISOLATED_VALUE_EG: i16 = -15;
const PAWN_BACKWARD_VALUE_MG: i16 = -10;
const PAWN_BACKWARD_VALUE_EG: i16 = -25;
const PAWN_SUPPORTED_VALUE_MG: i16 = 8;
const PAWN_SUPPORTED_VALUE_EG: i16 = 0;
const PAWN_ATTACK_CENTER_MG: i16 = 5;
const PAWN_ATTACK_CENTER_EG: i16 = 0;
pub struct PawnEvaluation {
    amount_of_pawns: i16,
    doubled_pawns: i16,
    isolated_pawns: i16,
    backwards_pawns: i16,
    supported_pawns: i16,
    center_attack_pawns: i16,
}

impl Evaluation for PawnEvaluation {
    fn eval_mg(&self) -> i16 {
        let mut res = 0;
        res += self.amount_of_pawns * PAWN_PIECE_VALUE_MG;
        res += self.doubled_pawns * PAWN_DOUBLED_VALUE_MG;
        res += self.isolated_pawns * PAWN_ISOLATED_VALUE_MG;
        res += self.backwards_pawns * PAWN_BACKWARD_VALUE_MG;
        res += self.supported_pawns * PAWN_SUPPORTED_VALUE_MG;
        res += self.center_attack_pawns * PAWN_ATTACK_CENTER_MG;
        res
    }
    fn eval_eg(&self) -> i16 {
        let mut res = 0;
        res += self.amount_of_pawns * PAWN_PIECE_VALUE_EG;
        res += self.doubled_pawns * PAWN_DOUBLED_VALUE_EG;
        res += self.isolated_pawns * PAWN_ISOLATED_VALUE_EG;
        res += self.backwards_pawns * PAWN_BACKWARD_VALUE_EG;
        res += self.supported_pawns * PAWN_SUPPORTED_VALUE_EG;
        res += self.center_attack_pawns * PAWN_ATTACK_CENTER_EG;
        res
    }
}

impl MidGameDisplay for PawnEvaluation {
    fn display_mg(&self) -> String {
        let mut res_str = String::new();
        res_str.push_str("\tPawns-MidGame\n");
        res_str.push_str(&format!(
            "\t\tAmount of Pawns: {} -> {}\n",
            self.amount_of_pawns,
            self.amount_of_pawns * PAWN_PIECE_VALUE_MG
        ));
        res_str.push_str(&format!(
            "\t\tDoubled Pawns:   {} -> {}\n",
            self.doubled_pawns,
            self.doubled_pawns * PAWN_DOUBLED_VALUE_MG
        ));
        res_str.push_str(&format!(
            "\t\tIsolated Pawns:  {} -> {}\n",
            self.isolated_pawns,
            self.isolated_pawns * PAWN_ISOLATED_VALUE_MG
        ));
        res_str.push_str(&format!(
            "\t\tBackwards Pawns: {} -> {}\n",
            self.backwards_pawns,
            self.backwards_pawns * PAWN_BACKWARD_VALUE_MG
        ));
        res_str.push_str(&format!(
            "\t\tSupported Pawns: {} -> {}\n",
            self.supported_pawns,
            self.supported_pawns * PAWN_SUPPORTED_VALUE_MG
        ));
        res_str.push_str(&format!(
            "\t\tCenter Attack : {} -> {}\n",
            self.center_attack_pawns,
            self.center_attack_pawns * PAWN_ATTACK_CENTER_MG
        ));
        res_str.push_str(&format!("\tSum: {}\n", self.eval_mg()));
        res_str
    }
}

impl EndGameDisplay for PawnEvaluation {
    fn display_eg(&self) -> String {
        let mut res_str = String::new();
        res_str.push_str("\tPawns-EndGame\n");
        res_str.push_str(&format!(
            "\t\tAmount of Pawns: {} -> {}\n",
            self.amount_of_pawns,
            self.amount_of_pawns * PAWN_PIECE_VALUE_EG
        ));
        res_str.push_str(&format!(
            "\t\tDoubled Pawns:   {} -> {}\n",
            self.doubled_pawns,
            self.doubled_pawns * PAWN_DOUBLED_VALUE_EG
        ));
        res_str.push_str(&format!(
            "\t\tIsolated Pawns:  {} -> {}\n",
            self.isolated_pawns,
            self.isolated_pawns * PAWN_ISOLATED_VALUE_EG
        ));
        res_str.push_str(&format!(
            "\t\tBackwards Pawns: {} -> {}\n",
            self.backwards_pawns,
            self.backwards_pawns * PAWN_BACKWARD_VALUE_EG
        ));
        res_str.push_str(&format!(
            "\t\tSupported Pawns:  {} -> {}\n",
            self.supported_pawns,
            self.supported_pawns * PAWN_SUPPORTED_VALUE_EG
        ));
        res_str.push_str(&format!(
            "\t\tCenter Attack :  {} -> {}\n",
            self.center_attack_pawns,
            self.center_attack_pawns * PAWN_ATTACK_CENTER_EG
        ));
        res_str.push_str(&format!("\tSum: {}\n", self.eval_eg()));
        res_str
    }
}

pub fn pawn_eval_white(
    w_pawns: u64,
    w_pawns_front_span: u64,
    w_pawn_attack_span: u64,
    black_pawn_attacks: u64,
    white_pawn_attacks: u64,
) -> PawnEvaluation {
    let file_fill = bitboards::file_fill(w_pawns);
    let amount_of_pawns = w_pawns.count_ones() as i16;
    let doubled_pawns = pawns_behind_own(w_pawns, w_pawns_front_span) as i16;
    let isolated_pawns = isolated_pawns(w_pawns, file_fill) as i16;
    let backwards_pawns = w_backwards(w_pawns, w_pawn_attack_span, black_pawn_attacks) as i16;
    let supported_pawns = (w_pawns & white_pawn_attacks).count_ones() as i16;
    let center_attack_pawns = ((bitboards::south_east_one(*bitboards::INNER_CENTER)
        | bitboards::south_west_one(*bitboards::INNER_CENTER))
        & w_pawns)
        .count_ones() as i16;
    PawnEvaluation {
        amount_of_pawns,
        doubled_pawns,
        isolated_pawns,
        backwards_pawns,
        supported_pawns,
        center_attack_pawns,
    }
}

pub fn pawn_eval_black(
    b_pawns: u64,
    b_pawns_front_span: u64,
    b_pawn_attack_span: u64,
    white_pawn_attacks: u64,
    black_pawn_attacks: u64,
) -> PawnEvaluation {
    let file_fill = bitboards::file_fill(b_pawns);
    let amount_of_pawns = b_pawns.count_ones() as i16;
    let doubled_pawns = pawns_behind_own(b_pawns, b_pawns_front_span) as i16;
    let isolated_pawns = isolated_pawns(b_pawns, file_fill) as i16;
    let backwards_pawns = b_backwards(b_pawns, b_pawn_attack_span, white_pawn_attacks) as i16;
    let supported_pawns = (b_pawns & black_pawn_attacks).count_ones() as i16;
    let center_attack_pawns = ((bitboards::north_east_one(*bitboards::INNER_CENTER)
        | bitboards::north_west_one(*bitboards::INNER_CENTER))
        & b_pawns)
        .count_ones() as i16;
    PawnEvaluation {
        amount_of_pawns,
        doubled_pawns,
        isolated_pawns,
        backwards_pawns,
        supported_pawns,
        center_attack_pawns,
    }
}

pub fn w_backwards(w_pawns: u64, w_pawn_attack_span: u64, black_pawn_attacks: u64) -> u32 {
    let stops = w_pawns << 8;
    (stops & black_pawn_attacks & !w_pawn_attack_span).count_ones()
}

pub fn b_backwards(b_pawns: u64, b_pawn_attack_span: u64, white_pawn_attacks: u64) -> u32 {
    let stops = b_pawns >> 8;
    (stops & white_pawn_attacks & !b_pawn_attack_span).count_ones()
}

pub fn pawns_behind_own(pawns: u64, front_span: u64) -> u32 {
    (pawns & front_span).count_ones()
}

pub fn isolated_pawns(pawns: u64, file_fill: u64) -> u32 {
    (pawns & !bitboards::west_one(file_fill) & !bitboards::east_one(file_fill)).count_ones()
}
