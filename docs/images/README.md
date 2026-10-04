# Note previews

These PNGs show the README JSON examples as question/answer pairs. They are documentation previews, not screenshots of a live Anki collection. Standard note examples follow Anki's stock template behavior; the custom French template is illustrative. The Japanese preview reads the CSS and templates from `src/anki.rs`.

To regenerate, use Node.js with Playwright available and a Chromium browser:

```sh
node scripts/render-note-examples.cjs
```

Run from any directory. If Playwright is installed outside the project, set `NODE_PATH` to its `node_modules` directory. If its bundled browser is unavailable, set `CHROMIUM_PATH` to a Chrome/Chromium executable. This tooling is only for documentation and adds no CLI runtime dependency. It reads the current README examples, renders them in a temporary browser session, and writes the seven PNGs here. It never contacts AnkiConnect or imports notes.
