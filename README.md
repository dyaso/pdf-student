# pdf-studier

🚧 Very bare bones cross-platform PDF reader for non-fiction books where you want to jump around a lot.

This is a simplified clone of [SumatraPDF](https://en.wikipedia.org/wiki/Sumatra_PDF) in Rust/[Druid](https://linebender.org/druid/) with a few different features:
 
 * quick jumps between multiple bookmarks
 * easy page cropping to trim wide margins and watermarks
 * a panel showing an overview of page tags in place of a scrollbar
 * arbitrary rectangle colour inversion, so you can view a page in quiet light-text-against-dark-background-o-vision but still see illustrations in as-printed colours
 * if you use Dropbox or a similar service you can automatically sync your page positions, tags, and bookmarks between machines

It can't do most things you might want from a PDF reader like edit forms, print, or select and copy text; it's just for reading books from screens.

### Installation

On NixOS 'unstable', run `nix-env -if default.nix`.

To build from source on any system, install the `cargo` command [somehow](https://rustup.rs/) then run `cargo run --release` in the terminal (or leave out `--release` for a much slower, larger, debug build) and copy the executable file `./target/release/pdf-student` to somewhere listed in your shell's [search `PATH` environment variable](https://en.wikipedia.org/wiki/PATH_(variable)).

Coming soon hopefully: pre-built mac and Windows binaries.

### Instructions

It's meant to be mostly keyboard operated (use arrow keys to navigate, ctrl-+ and - to zoom, PgUp / PgDown to go to previous/next tagged page), with keyboard shortcuts shown in the pop-up context menu. Mousing over the page overview panel temporarily shows pages in the main view to make browsing around quicker.

Double click to edit page cropping.

Bookmarks are single letters: type a letter to assign it to the page you're on, then type it again later to jump back to that page. Press &lt;SPACE> to erase a bookmark.

Press &lt;BACKSPACE> to go back to the page you were on before a jump, like a browser's Back button.

Pages can be tagged with coloured dots by pressing the digits 1-9, or 0 to clear the tags. Pressing "," or "." will move to the previous or next page and copy the current page's tags to it, useful for marking off a section as you read it.

Note that if you apply a tag or bookmark while mousing over a hyperlink, it's the page at the other end of the link that receives the tag or bookmark; this makes going over the Contents page and quickly adding markers to chapters' first pages easier.

### Flaws and missing features

Currently pages are only rendered when they first appear on screen, so if you resize the window after that they can become blurry. Hit F5 to refresh.

There's no zoom-in function, but you can get a zoomed out effect by repositioning the overview panel (press &lt;TAB>) alongside the scroll direction (SHIFT-&lt;TAB>) and resizing it.

A PDF file's built-in table of contents, should it exist, is ignored.