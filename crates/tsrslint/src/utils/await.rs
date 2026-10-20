//! Port of internal/utils/await.go.

use super::*;

/// await.go GetAwaitTokenRemovalRange: the token at `pos` and its trailing whitespace, without comments.
pub fn get_await_token_removal_range(source_file: P<SourceFile>, pos: TextPos) -> (i32, i32) {
    let token_range = tsrs_scanner::get_range_of_token_at_position(source_file, pos_u32(pos));
    let end = tsrs_scanner::skip_trivia_ex(
        source_file.text(),
        pos_u32(token_range.end()),
        Some(&tsrs_scanner::SkipTriviaOptions {
            stop_at_comments: true,
            ..Default::default()
        }),
    );
    (pos_i32(token_range.pos()), end as i32)
}
