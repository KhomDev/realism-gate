#![no_main]

use libfuzzer_sys::fuzz_target;

// The property: the declaration frontend never panics on any input.
fuzz_target!(|data: &[u8]| {
    realism_gate_fuzz::declaration(data);
});
