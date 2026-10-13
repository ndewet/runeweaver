//! Checks that the shim does not require std or an allocator.

#![no_std]
#![no_main]

use logos::Logos;

#[derive(Logos)]
#[logos(utf8 = false)]
enum Token {
    #[token(b"\xff")]
    Byte,
}

pub fn scan_byte() -> bool {
    matches!(Token::lexer(b"\xff").next(), Some(Ok(Token::Byte)))
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {}
}
