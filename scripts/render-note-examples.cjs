// Documentation-only renderer. Requires Playwright; never connects to Anki.
// NODE_PATH=/path/to/node_modules node scripts/render-note-examples.cjs
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require('playwright');

const root = path.resolve(__dirname, '..');
const readme = fs.readFileSync(path.join(root, 'README.md'), 'utf8');
const anki = fs.readFileSync(path.join(root, 'src/anki.rs'), 'utf8');
const escape = value => String(value).replace(/[&<>"']/g, char => ({
  '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;'
}[char]));
function example(heading) {
  const section = readme.split(heading)[1];
  const value = JSON.parse(section.match(/```json\n([\s\S]*?)\n```/)[1]);
  return Array.isArray(value) ? value[0] : value;
}
function pair(label, question, answer) {
  return `<div class="pair"><div class="direction">${escape(label)}</div>
    <section><div class="label">QUESTION</div><div class="card">${question}</div></section>
    <section><div class="label">ANSWER REVEALED</div><div class="card">${answer}</div></section></div>`;
}
function basicPair(card, reverse = false) {
  const front = escape(reverse ? card.Back : card.Front);
  const back = escape(reverse ? card.Front : card.Back);
  return pair(reverse ? 'Card 2 · Back → Front' : 'Card 1 · Front → Back', front, `${front}<hr id="answer">${back}`);
}
function panel(id, title, description, content) {
  return `<article id="${id}"><header><div class="eyebrow">ANKI-AUTO · NOTE PREVIEW</div>
    <h1>${escape(title)}</h1><p>${escape(description)}</p></header>${content}</article>`;
}
const basic = example('### Basic\n');
const reversed = example('### Basic (and reversed card)');
const optional = example('### Basic (optional reversed card)');
const typed = example('### Basic (type in the answer)');
const cloze = example('### Cloze');
const custom = example('### Your own note type');
const jp = example('## CLI');
const japaneseCss = anki.match(/const CSS: &str = r#"([\s\S]*?)"#;/)[1];
function highlight(word, sentence) {
  let match = sentence.includes(word) ? word : null;
  const chars = [...word];
  for (let keep = chars.length - 1; match === null && keep >= 2; keep--) {
    const stem = chars.slice(0, keep).join('');
    if (sentence.includes(stem)) match = stem;
  }
  return match === null ? escape(sentence) : escape(sentence).replace(escape(match), `<b>${escape(match)}</b>`);
}
function japaneseTemplate(name, fields) {
  return anki.match(new RegExp(`const ${name}: &str = r#"([\\s\\S]*?)"#;`))[1]
    .replace(/\{\{#SentenceMeaning\}\}([\s\S]*?)\{\{\/SentenceMeaning\}\}/g, (_, content) => fields.SentenceMeaning ? content : '')
    .replace(/\{\{(\w+)\}\}/g, (_, field) => fields[field] || '');
}
const jpFields = {
  Word: escape(jp.word), Reading: escape(jp.reading), Meaning: escape(jp.meaning),
  Sentence: highlight(jp.word, jp.sentence), SentenceMeaning: escape(jp.sentence_meaning || ''),
};
const jpFront = japaneseTemplate('FRONT_TEMPLATE', jpFields);
const jpBack = japaneseTemplate('BACK_TEMPLATE', { ...jpFields, FrontSide: jpFront });
const clozeQuestion = escape(cloze.Text).replace(/\{\{c1::(.*?)\}\}/g, '<span class="cloze">[...]</span>');
const clozeAnswer = escape(cloze.Text).replace(/\{\{c1::(.*?)\}\}/g, '<span class="cloze">$1</span>');
const panels = [
  panel('basic', 'Basic', 'One JSON note → one review card', basicPair(basic)),
  panel('reversed', 'Basic (and reversed card)', 'One JSON note → two review cards', basicPair(reversed) + basicPair(reversed, true)),
  panel('optional-reversed', 'Basic (optional reversed card)', 'Add Reverse = "yes" → both directions. Empty or omitted → Card 1 only.', basicPair(optional) + basicPair(optional, true)),
  panel('typed', 'Basic (type in the answer)', 'One review card · example with a correctly typed answer',
    pair('Card 1 · Type the answer', `${escape(typed.Front)}<input aria-label="Your answer" placeholder="Type your answer…" readonly>`,
      `${escape(typed.Front)}<hr id="answer"><span class="correct">${escape(typed.Back)}</span>`)),
  panel('cloze', 'Cloze', '{{c1::France}} hides France until the answer is revealed',
    pair('Card 1 · Fill in the blank', clozeQuestion, `${clozeAnswer}<br><span class="extra">${escape(cloze['Back Extra'])}</span>`)),
  panel('custom', 'French Vocabulary', 'Illustrative custom template · your Anki templates determine the layout',
    pair('Card 1 · French → English', escape(custom.French), `${escape(custom.French)}<hr id="answer">${escape(custom.English)}<div class="extra">${escape(custom.Example)}</div>`)),
  panel('japanese', 'jp-vocab', 'Built-in Japanese format · automatic sentence highlighting',
    `<div class="japanese">${pair('Card 1 · Word → Meaning', jpFront, jpBack)}</div>`),
];
const html = `<!doctype html><html lang="en"><meta charset="utf-8"><title>Anki note examples</title><style>
* { box-sizing: border-box; }
body { margin: 0; background: #fff; font-family: Arial, sans-serif; color: #202124; }
article { width: 960px; padding: 28px; background: #f4f5f6; margin: 0 0 24px; }
header { margin-bottom: 24px; }
.eyebrow { font-size: 11px; letter-spacing: 1.4px; color: #647078; font-weight: 700; }
h1 { font-size: 24px; margin: 10px 0 8px; }
p { margin: 0; color: #586169; font-size: 14px; line-height: 1.5; }
.pair { display: grid; grid-template-columns: 1fr 1fr; gap: 12px 18px; margin-top: 18px; }
.direction { grid-column: 1 / -1; font-size: 13px; font-weight: 700; color: #46535d; }
section { background: white; border: 1px solid #d6dce1; border-radius: 8px; overflow: hidden; }
.label { padding: 12px 16px; font-size: 10px; font-weight: 700; letter-spacing: 1px; color: #63707a; border-bottom: 1px solid #e5e8eb; }
.card { font-family: Arial, sans-serif; font-size: 20px; text-align: center; color: black; background-color: white; padding: 30px 22px; min-height: 145px; line-height: 1.5; }
hr { border: 0; border-top: 1px solid #a0a0a0; margin: 18px 0; }
input { display: block; width: 90%; margin: 18px auto 0; padding: 8px; font: 17px monospace; border: 1px solid #999; background: white; }
.correct { color: #000; background: #c0ffc0; font-family: monospace; }
.cloze { font-weight: bold; color: blue; }
.extra { display: block; margin-top: 16px; }
div.extra { font-size: 17px; color: #586169; }
${japaneseCss.replaceAll('.card', '.japanese .card').replaceAll('.word', '.japanese .word').replaceAll('.reading', '.japanese .reading').replaceAll('.meaning', '.japanese .meaning').replaceAll('.sentence', '.japanese .sentence').replaceAll('hr#answer', '.japanese hr#answer')}
</style>${panels.join('\n')}</html>`;

(async () => {
  const browser = await chromium.launch({
    ...(process.env.CHROMIUM_PATH ? { executablePath: process.env.CHROMIUM_PATH } : {})
  });
  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 1000 }, deviceScaleFactor: 2 });
    await page.setContent(html);
    await page.evaluate(() => document.fonts.ready);
    for (const id of ['basic', 'reversed', 'optional-reversed', 'typed', 'cloze', 'custom', 'japanese']) {
      await page.locator(`#${id}`).screenshot({ path: path.join(root, 'docs/images', `${id}.png`) });
      console.log(`Rendered docs/images/${id}.png`);
    }
  } finally {
    await browser.close();
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
