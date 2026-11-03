# Formatted local answers and app identity

2026-10-23. Local AI answers are displayed using a bounded CommonMark parser with native egui widgets. Model text is data: no HTML engine, executable tools, fetched images, clickable external routes or persistent answer cache. Normal Markdown emphasis, lists, headings, tables, quotes and code are supported; the exact `<u>` and `</u>` tags provide underline. Copy answer preserves the original model text.

The local spreadsheet edition is TablesCraft. Its internal Rust crates, legacy profile directory and workbook ProgId retain GridCraft identifiers for compatibility. Windows titles, dialogs, executable, installer, shortcuts and Open with labels use TablesCraft. Original upstream attribution remains intact.

App identity colors: Word blue, Tables green, Deck orange, PDF red on white. These colors affect app chrome and icons, not document content. Interface text uses hyphens instead of em dashes. Original D: repositories remain untouched.

2026-10-26 layout correction: the AI launcher lives in the existing status bar (PDF: existing mode toolbar). Opening and closing the floating AI panel no longer reserves an extra editor row. Word launcher and selection-menu labels participate in interface translation. A regression check verifies unchanged editor space when the panel is closed.
