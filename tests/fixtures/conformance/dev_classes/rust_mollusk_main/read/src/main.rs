// DEV class "Mollusk fn main". Synthetic: a PoC binary that builds a Mollusk
// harness in `fn main` and runs the exploit through it. It must be read as
// one test, though the path is src/main.rs.
use mollusk_svm::Mollusk;
use solana_sdk::instruction::{AccountMeta, Instruction};
use solana_sdk::pubkey::Pubkey;

fn main() {
    let program_id = Pubkey::new_unique();
    let mollusk = Mollusk::new(&program_id, "vault");
    let attacker = Pubkey::new_unique();
    let ix = Instruction::new_with_bytes(program_id, &[1], vec![AccountMeta::new(attacker, true)]);
    let result = mollusk.process_instruction(&ix, &[]);
    assert!(result.program_result.is_ok());
}
