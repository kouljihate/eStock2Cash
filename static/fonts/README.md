# Fonts

The app serves this directory at `/static/fonts/<file>` (see `font_asset` in
`src/routes.rs`), and `static/app.css` declares the faces with `@font-face`:

| File | Used for | Declared as |
| --- | --- | --- |
| `Comfortaa-Light.ttf` (300) | Latin UI weight | `Comfortaa` |
| `Comfortaa-Regular.ttf` (400) | Latin UI body | `Comfortaa` |
| `Comfortaa-Bold.ttf` (700) | Latin UI headings | `Comfortaa` |
| `VIP-Rawy-Regular.ttf` (400) | Arabic UI body | `VIP Rawy` |
| `VIP-Rawy-Bold.ttf` (700) | Arabic UI headings | `VIP Rawy` |
| `arabic.ttf` | Arabic **PDF** text | embedded by `src/fonts.rs` |

If a file is missing the browser falls back to the system stack, so the UI still
works — it just loses the branding. Arabic PDFs fall back to English/French
labels without `arabic.ttf`; point `PDF_ARABIC_FONT` at another TrueType file
to swap it.

## Licensing

- `Comfortaa-*.ttf` — SIL Open Font License, see `OFL-Comfortaa.txt`.
- `arabic.ttf` — Amiri, SIL Open Font License, see `OFL.txt`.
- `VIP-Rawy-*.ttf` — commercial face provided by the operator. It is **not**
  committed to the repository (see `.gitignore`); copy it here when deploying a
  new environment.
