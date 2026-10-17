// Pure parser for AI playlist suggestions (clipboard round-trip).
// No imports, no DOM, no IPC: fully unit-testable. See src/dev/aiSuggestTest.mjs.

export interface Suggestion {
  title: string;
  /** May be empty when the line carried only a title. */
  artist: string;
  /** Original line number (0-based) the suggestion was parsed from. */
  line: number;
}

/**
 * Parse AI output into song suggestions.
 *
 * Accepts "Title | Artist" and "Title - Artist" per line. Strips list
 * markers ("1.", "1)", "-", "*"), surrounding quotes and whitespace. Blank
 * lines and obvious preamble/label junk are ignored. The first separator
 * wins ("Title - Sub - Artist" => title "Title", artist "Sub - Artist") so
 * titles containing dashes survive; the pipe separator beats the dash when
 * both appear (markdown tables parse correctly).
 */
export function parseSuggestions(input: string): Suggestion[] {
  const out: Suggestion[] = [];
  const lines = input.split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const parsed = parseLine(lines[i], i);
    if (parsed) out.push(parsed);
  }
  return out;
}

/** Formatting-hygiene rules: junk shapes a bare title line must not match. */
/** Markdown table divider: | --- | ----------- | */
const DIVIDER_RE = /^[-: |]+$/;

const JUNK_PATTERNS: RegExp[] = [
  /^here (?:are|is)\b/i,
  /^(?:sure|okay|ok|great|got it|certainly)\b/i,
  /^based on\b/i,
  /^i(?:'ve| have| will|'ll|'d| would)? (?:picked|selected|chosen|created|made|compiled|curated|put together|found)/i,
  /^these (?:are|should)\b/i,
  /^enjoy\b/i,
  /^hope (?:this|these|you)/i,
  /^(?:top )?\d* ?songs? (?:that|which|to)\b/i,
  /^\w+(?: \w+){0,2}:$/, // bare section labels ("Suggestions:", "Here you go:")
  /^(?:\d+|[a-z])[.)]?$/, // lone numbers / letters (list artifacts)
  /^(?:song|track|title|artist|no|#)\b[.:#-]*$/i, // table headers
];

function parseLine(rawLine: string, lineNo: number): Suggestion | null {
  const line = stripMarkdown(rawLine);
  if (!line) return null;

  // Separator detection BEFORE stripping list markers: markdown tables use
  // "|" as the only visual separator, and a leading "| 1." sequence would
  // otherwise be confused with a numbered list.
  const pipeIdx = line.indexOf("|");
  let title = "";
  let artist = "";

  if (pipeIdx !== -1) {
    // Pipe format wins over dash when both exist (markdown table rows).
    const rest = line.slice(pipeIdx + 1);
    const second = rest.indexOf("|");
    if (second !== -1) {
      title = line.slice(0, pipeIdx);
      artist = rest.slice(0, second);
    } else {
      title = line.slice(0, pipeIdx);
      artist = rest;
    }
  } else {
    const dash = /\s[-\u2013\u2014]\s/.exec(line);
    if (!dash) return null;
    title = line.slice(0, dash.index);
    artist = line.slice(dash.index + dash[0].length);
  }

  title = cleanTitle(title);
  artist = cleanArtist(artist);
  if (!title) return null;
  // Markdown table divider cells are pure punctuation, never a song.
  if (DIVIDER_RE.test(title) || DIVIDER_RE.test(artist)) return null;
  // Artist cells: only table-header words disqualify ("Song | Artist" from a
  // pasted header row). Numbers stay - years as artist ("Nights | 2016") are
  // plausible AI output, and the search fallback still resolves the title.
  if (artist && isHeaderWord(artist)) return null;

  // A bare title with junk shape ("Sure, here are...", a lone "1.", a
  // "Song | Artist" header remnant) is preamble, not a song.
  if (!artist && isJunk(title)) return null;

  return { title, artist, line: lineNo };
}

function stripMarkdown(raw: string): string {
  let s = raw.trim();
  if (!s) return "";
  // Table row: leading/trailing pipes are table borders.
  if (s.startsWith("|")) s = s.slice(1);
  if (s.endsWith("|")) s = s.slice(0, -1);
  s = s.trim();
  if (!s) return "";
  // List markers: "1." "1)" "1 -" "-" "*" "+". Only stripped when followed by
  // a space so it cannot eat a dash that separates title from artist.
  s = s.replace(/^(?:\d{1,3}[.)]+\s+|[*+\u2022\u2023\u25E6\u2043\u2219-]\s+)/, "");
  // Whole-cell emphasis first, then surrounding quotes. Anchored pairs only:
  // a lazy `(.+)\*` across a "|" cell boundary would eat the separator's
  // neighbors ("**A** | **B**" would lose its outer asterisks unevenly).
  s = s.replace(/^\*\*([^*|]+)\*\*/, "$1");
  s = s.replace(/^\*([^*|]+)\*/, "$1");
  s = s.replace(/^["'\u201C\u201D\u2018\u2019]+/, "").replace(/["'\u201C\u201D\u2018\u2019]+$/, "");
  return s.trim();
}

function stripInlineEmphasis(s: string): string {
  s = s.trim();
  // Wrapping bold on a cell: "**Bold Artist**" -> "Bold Artist" (single pass,
  // not recursive - nested emphasis does not survive a copy-paste round-trip).
  s = s.replace(/^\*\*(.*)\*\*$/, "$1");
  // Inner pairs (any position): **bold** / *italic*
  s = s.replace(/\*\*([^*]+)\*\*/g, "$1");
  s = s.replace(/\*([^*]+)\*/g, "$1");
  return s.trim();
}

function cleanTitle(s: string): string {
  return stripInlineEmphasis(stripMarkdown(s));
}

function cleanArtist(s: string): string {
  return stripInlineEmphasis(stripMarkdown(s));
}

/** Junk detection for bare-title lines (preamble, list artifacts, headers). */
function isJunk(s: string): boolean {
  if (!s) return false;
  return JUNK_PATTERNS.some((re) => re.test(s));
}

/** Table-header words: an artist cell holding "Artist"/"Song" is a pasted
 * header row, not a song. Years and other short cells stay. */
function isHeaderWord(s: string): boolean {
  return /^(?:song|track|title|artist|no|#)\b[.:#-]*$/i.test(s);
}
