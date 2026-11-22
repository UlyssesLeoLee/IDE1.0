// Safe Myers Diff, Region-Cluster 3-Way Merge, Syntax Integrity Check & Official NEMA v1.5 Parser/Merger
// Aligned 1:1 with Rust `editcore::diff`, `ide-mcp::merge::algo`, and `nema-codec` (P1-02, P1-06).

const MAX_EDIT_DISTANCE = 1500;
const AXES_5W1H = ["What", "Why", "How", "Who", "When", "Where"];
const COLUMNS_GRAPH = ["Element", "Relation", "Description"];

export function splitLines(s) {
  if (!s) return [];
  return s.match(/[^\n]*\n|[^\n]+$/g) || [];
}

function toLinesArray(v) {
  return Array.isArray(v) ? v : splitLines(String(v || ""));
}

function myers(a, b) {
  const n = a.length;
  const m = b.length;
  if (n === 0 && m === 0) return [];
  if (n === 0) return Array.from({ length: m }, (_, j) => ({ op: "ins", a: 0, b: j }));
  if (m === 0) return Array.from({ length: n }, (_, i) => ({ op: "del", a: i, b: 0 }));

  const dmax = Math.min(n + m, MAX_EDIT_DISTANCE);
  const off = dmax + 1;
  const v = new Int32Array(2 * dmax + 3);
  const trace = [];
  let dfound = -1;

  outer: for (let d = 0; d <= dmax; d++) {
    for (let k = -d; k <= d; k += 2) {
      const ki = off + k;
      let x = k === -d || (k !== d && v[ki - 1] < v[ki + 1]) ? v[ki + 1] : v[ki - 1] + 1;
      let y = x - k;
      while (x < n && y < m && a[x] === b[y]) {
        x++;
        y++;
      }
      v[ki] = x;
      if (x >= n && y >= m) {
        trace.push(v.slice(off - d, off + d + 1));
        dfound = d;
        break outer;
      }
    }
    trace.push(v.slice(off - d, off + d + 1));
  }

  if (dfound < 0) return null;

  const out = [];
  let x = n;
  let y = m;
  for (let d = dfound; d >= 0; d--) {
    if (d === 0) {
      while (x > 0 && y > 0) {
        x--;
        y--;
        out.push({ op: "eq", a: x, b: y });
      }
      break;
    }
    const vp = trace[d - 1];
    const at = (k) => vp[k + d - 1];
    const k = x - y;
    const prevK = k === -d || (k !== d && at(k - 1) < at(k + 1)) ? k + 1 : k - 1;
    const prevX = at(prevK);
    const prevY = prevX - prevK;
    while (x > prevX && y > prevY) {
      x--;
      y--;
      out.push({ op: "eq", a: x, b: y });
    }
    if (x === prevX) {
      y--;
      out.push({ op: "ins", a: x, b: y });
    } else {
      x--;
      out.push({ op: "del", a: x, b: y });
    }
  }
  return out.reverse();
}

export function diffLines(aInput, bInput) {
  const a = toLinesArray(aInput);
  const b = toLinesArray(bInput);
  let pre = 0;
  while (pre < a.length && pre < b.length && a[pre] === b[pre]) pre++;
  let suf = 0;
  while (
    suf < a.length - pre &&
    suf < b.length - pre &&
    a[a.length - 1 - suf] === b[b.length - 1 - suf]
  ) {
    suf++;
  }
  const am = a.slice(pre, a.length - suf);
  const bm = b.slice(pre, b.length - suf);

  const steps = [];
  for (let i = 0; i < pre; i++) steps.push({ op: "eq", a: i, b: i });
  let coarse = false;
  const mid = myers(am, bm);
  if (mid) {
    for (const s of mid) steps.push({ op: s.op, a: s.a + pre, b: s.b + pre });
  } else {
    coarse = true;
    for (let i = 0; i < am.length; i++) steps.push({ op: "del", a: pre + i, b: pre });
    for (let j = 0; j < bm.length; j++) steps.push({ op: "ins", a: pre + am.length, b: pre + j });
  }
  for (let i = 0; i < suf; i++) {
    steps.push({ op: "eq", a: a.length - suf + i, b: b.length - suf + i });
  }
  return { steps, coarse };
}

export function regions(steps) {
  const out = [];
  let cur = null;
  for (const s of steps) {
    if (s.op === "eq") {
      if (cur) {
        out.push(cur);
        cur = null;
      }
      continue;
    }
    if (!cur) cur = { a0: s.a, a1: s.a, b0: s.b, b1: s.b };
    if (s.op === "del") cur.a1 = s.a + 1;
    else cur.b1 = s.b + 1;
  }
  if (cur) out.push(cur);
  return out;
}

export function formatUnifiedDiff(aInput, bInput, nameA = "a", nameB = "b", context = 3) {
  const aLines = Array.isArray(aInput) ? aInput.map((l) => (l.endsWith("\n") ? l : `${l}\n`)) : splitLines(aInput);
  const bLines = Array.isArray(bInput) ? bInput.map((l) => (l.endsWith("\n") ? l : `${l}\n`)) : splitLines(bInput);
  const { steps } = diffLines(aLines, bLines);
  const regs = regions(steps);
  if (regs.length === 0) return "";

  const hunks = [];
  for (const r of regs) {
    const startA = Math.max(0, r.a0 - context);
    const endA = Math.min(aLines.length, r.a1 + context);
    const startB = Math.max(0, r.b0 - context);
    const endB = Math.min(bLines.length, r.b1 + context);
    if (hunks.length > 0 && startA <= hunks[hunks.length - 1].endA) {
      const prev = hunks[hunks.length - 1];
      prev.endA = endA;
      prev.endB = endB;
      prev.regs.push(r);
    } else {
      hunks.push({ startA, endA, startB, endB, regs: [r] });
    }
  }

  let out = `--- ${nameA}\n+++ ${nameB}\n`;
  for (const h of hunks) {
    const body = [];
    let ai = h.startA;
    let bi = h.startB;
    for (const r of h.regs) {
      while (ai < r.a0) {
        body.push(` ${aLines[ai].replace(/[\r\n]+$/, "")}`);
        ai++;
        bi++;
      }
      for (let i = r.a0; i < r.a1; i++) body.push(`-${aLines[i].replace(/[\r\n]+$/, "")}`);
      for (let j = r.b0; j < r.b1; j++) body.push(`+${bLines[j].replace(/[\r\n]+$/, "")}`);
      ai = r.a1;
      bi = r.b1;
    }
    while (ai < h.endA) {
      body.push(` ${aLines[ai].replace(/[\r\n]+$/, "")}`);
      ai++;
    }
    const lenA = h.endA - h.startA;
    const lenB = h.endB - h.startB;
    out += `@@ -${h.startA + 1},${lenA} +${h.startB + 1},${lenB} @@\n${body.join("\n")}\n`;
  }
  return out;
}

function regionsOverlap(a, b) {
  if (a.a0 === a.a1 && b.a0 === b.a1 && a.a0 === b.a0) return true;
  return !(a.a1 <= b.a0 || b.a1 <= a.a0);
}

function reconstructSideSlice(baseLines, sideLines, clusterStart, clusterEnd, regs) {
  let out = "";
  let pos = clusterStart;
  for (const r of regs) {
    while (pos < r.a0 && pos < baseLines.length) out += baseLines[pos++];
    for (let j = r.b0; j < r.b1; j++) out += sideLines[j];
    pos = r.a1;
  }
  while (pos < clusterEnd && pos < baseLines.length) out += baseLines[pos++];
  return out;
}

export function threeWayMerge(baseInput, oursInput, theirsInput) {
  const baseLines = toLinesArray(baseInput);
  const oursLines = toLinesArray(oursInput);
  const theirsLines = toLinesArray(theirsInput);
  const baseText = baseLines.join("");
  const oursText = oursLines.join("");
  const theirsText = theirsLines.join("");

  if (oursText === theirsText) {
    return { clean: true, conflicts: 0, conflictHunks: [], lines: oursLines, text: oursText };
  }
  if (oursText === baseText) {
    return { clean: true, conflicts: 0, conflictHunks: [], lines: theirsLines, text: theirsText };
  }
  if (theirsText === baseText) {
    return { clean: true, conflicts: 0, conflictHunks: [], lines: oursLines, text: oursText };
  }

  const oursRegs = regions(diffLines(baseLines, oursLines).steps);
  const theirsRegs = regions(diffLines(baseLines, theirsLines).steps);

  let merged = "";
  const conflictHunks = [];
  let baseIdx = 0;
  let oi = 0;
  let ti = 0;

  while (oi < oursRegs.length || ti < theirsRegs.length) {
    const o = oursRegs[oi];
    const t = theirsRegs[ti];
    if (o && t && regionsOverlap(o, t)) {
      const o0Idx = oi;
      const t0Idx = ti;
      const cStart = Math.min(o.a0, t.a0);
      let cEnd = Math.max(o.a1, t.a1);
      oi++;
      ti++;
      while (true) {
        let extended = false;
        const span = { a0: cStart, a1: cEnd, b0: 0, b1: 0 };
        if (oi < oursRegs.length && regionsOverlap(span, oursRegs[oi])) {
          cEnd = Math.max(cEnd, oursRegs[oi].a1);
          oi++;
          extended = true;
        }
        if (ti < theirsRegs.length && regionsOverlap(span, theirsRegs[ti])) {
          cEnd = Math.max(cEnd, theirsRegs[ti].a1);
          ti++;
          extended = true;
        }
        if (!extended) break;
      }
      while (baseIdx < cStart && baseIdx < baseLines.length) merged += baseLines[baseIdx++];
      const oSlice = reconstructSideSlice(baseLines, oursLines, cStart, cEnd, oursRegs.slice(o0Idx, oi));
      const tSlice = reconstructSideSlice(baseLines, theirsLines, cStart, cEnd, theirsRegs.slice(t0Idx, ti));
      if (oSlice === tSlice) {
        merged += oSlice;
      } else {
        const nlO = oSlice && !oSlice.endsWith("\n") ? "\n" : "";
        const nlT = tSlice && !tSlice.endsWith("\n") ? "\n" : "";
        merged += `<<<<<<< HEAD (ours)\n${oSlice}${nlO}=======\n${tSlice}${nlT}>>>>>>> INCOMING (theirs)\n`;
        conflictHunks.push({
          base_line_start: cStart,
          base_line_end: cEnd,
          ours: oSlice,
          theirs: tSlice,
        });
      }
      baseIdx = cEnd;
    } else if (o && (!t || o.a1 <= t.a0)) {
      while (baseIdx < o.a0 && baseIdx < baseLines.length) merged += baseLines[baseIdx++];
      for (let j = o.b0; j < o.b1; j++) merged += oursLines[j];
      baseIdx = o.a1;
      oi++;
    } else if (t) {
      while (baseIdx < t.a0 && baseIdx < baseLines.length) merged += baseLines[baseIdx++];
      for (let j = t.b0; j < t.b1; j++) merged += theirsLines[j];
      baseIdx = t.a1;
      ti++;
    }
  }
  while (baseIdx < baseLines.length) merged += baseLines[baseIdx++];

  return {
    clean: conflictHunks.length === 0,
    conflicts: conflictHunks.length,
    conflictHunks,
    lines: splitLines(merged),
    text: merged,
  };
}

function checkDelimiterBalance(text, isRust) {
  const stack = [];
  let line = 1;
  for (let i = 0; i < text.length; i++) {
    const ch = text[i];
    if (ch === "\n") {
      line++;
      continue;
    }
    if (ch === "/" && text[i + 1] === "/") {
      while (i < text.length && text[i] !== "\n") i++;
      line++;
      continue;
    }
    if (ch === "/" && text[i + 1] === "*") {
      const startLine = line;
      let depth = 1;
      i += 2;
      while (i < text.length && depth > 0) {
        if (text[i] === "\n") line++;
        else if (isRust && text[i] === "/" && text[i + 1] === "*") {
          depth++;
          i++;
        } else if (text[i] === "*" && text[i + 1] === "/") {
          depth--;
          i += 2;
          if (depth === 0) break;
          continue;
        }
        i++;
      }
      if (depth > 0) return { ok: false, error: `Unclosed block comment from line ${startLine}` };
      i--;
      continue;
    }
    if (isRust && ch === "r") {
      let j = i + 1;
      let hashes = 0;
      while (text[j] === "#") {
        hashes++;
        j++;
      }
      if (text[j] === '"') {
        j++;
        while (j < text.length) {
          if (text[j] === "\n") line++;
          else if (text[j] === '"' && text.slice(j + 1, j + 1 + hashes) === "#".repeat(hashes)) {
            i = j + hashes;
            break;
          }
          j++;
        }
        continue;
      }
    }
    if (ch === '"' || (!isRust && (ch === "'" || ch === "`"))) {
      const q = ch;
      const startLine = line;
      i++;
      let closed = false;
      while (i < text.length) {
        if (text[i] === "\n") line++;
        else if (text[i] === "\\") {
          i += 2;
          continue;
        } else if (text[i] === q) {
          closed = true;
          break;
        }
        i++;
      }
      if (!closed) return { ok: false, error: `Unclosed string literal from line ${startLine}` };
      continue;
    }
    if (isRust && ch === "'") {
      if (text[i + 1] === "\\") {
        const end = text.indexOf("'", i + 2);
        if (end !== -1 && end - i < 16) i = end;
      } else if (i + 2 < text.length && text[i + 2] === "'" && text[i + 1] !== "'") {
        i += 2;
      }
      continue;
    }
    if (ch === "(" || ch === "{" || ch === "[") {
      stack.push({ ch, line });
    } else if (ch === ")" || ch === "}" || ch === "]") {
      const exp = ch === ")" ? "(" : ch === "}" ? "{" : "[";
      const top = stack.pop();
      if (!top) return { ok: false, error: `Unmatched '${ch}' at line ${line}` };
      if (top.ch !== exp) {
        return { ok: false, error: `Mismatched '${ch}' at line ${line} (expected close for '${top.ch}' from line ${top.line})` };
      }
    }
  }
  if (stack.length > 0) {
    const top = stack[stack.length - 1];
    return { ok: false, error: `Unclosed '${top.ch}' opened at line ${top.line}` };
  }
  return { ok: true };
}

export function checkSyntaxIntegrity(language, text) {
  for (const l of String(text || "").split("\n")) {
    if (l.startsWith("<<<<<<< ") || l.startsWith(">>>>>>> ")) {
      return { ok: false, error: "Conflict marker detected in candidate content" };
    }
  }
  const lang = String(language || "plain").toLowerCase();
  if (lang === "nema") return parseNemaStrict(text);
  if (lang === "json") {
    try {
      JSON.parse(text);
      return { ok: true };
    } catch (e) {
      return { ok: false, error: `JSON syntax error: ${e.message}` };
    }
  }
  if (["rust", "rs"].includes(lang)) return checkDelimiterBalance(text, true);
  if (["js", "javascript", "ts", "typescript", "c", "cpp", "java", "go"].includes(lang)) {
    return checkDelimiterBalance(text, false);
  }
  return { ok: true };
}

// --- Official NEMA v1.5 Strict Parser & Domain-Aware Merger (P1-06) ---

function splitUnescapedSpaces(line) {
  const tokens = [];
  let cur = "";
  for (let i = 0; i < line.length; i++) {
    const ch = line[i];
    if (ch === "\\" && i + 1 < line.length) {
      cur += ch + line[i + 1];
      i++;
      continue;
    }
    if (ch === "\t") throw new Error("TAB character prohibited in body; TAB reserved for domain control");
    if (ch === " ") {
      if (cur === "") throw new Error("Consecutive unescaped ASCII spaces or leading space in splitUnescapedSpaces");
      tokens.push(cur);
      cur = "";
      continue;
    }
    cur += ch;
  }
  if (cur === "") throw new Error("Trailing unescaped space");
  tokens.push(cur);
  return tokens;
}

function isValidDomainId(s) {
  return /^[A-Za-z][A-Za-z0-9_.-]*$/.test(s) || /^[1-9][0-9]*$/.test(s);
}

function isValidGraphIdent(s) {
  return /^[A-Za-z][A-Za-z0-9_.:-]*$/.test(s);
}

function parseDomainHeader(line) {
  let tail = null;
  let mode = null;
  let isV12 = false;

  // V1.2 / TAB domain header (`\t[\\?>]?ID REV` or ` [\\?>]?ID REV`)
  let rest = null;
  if (line.startsWith("\t")) rest = line.slice(1);
  else if (line.startsWith(" ") && !line.startsWith("  ")) rest = line.slice(1);
  if (rest === null) return null;

  // Check explicit v1.5 mode indicators first
  const sub = rest.startsWith("\\") ? rest.slice(1) : rest;
  if (sub.startsWith("|") || sub.startsWith("T")) {
    mode = "table";
    tail = sub.slice(1);
  } else if (sub.startsWith(">") || sub.startsWith("G")) {
    mode = "graph";
    tail = sub.slice(1);
  } else if (sub.startsWith("?")) {
    mode = "5w1h";
    tail = sub.slice(1);
  } else if (rest.startsWith("\\")) {
    // Legacy ` \1 1` -> defaults to table if digits/explicit, or v1.2
    mode = "table";
    tail = rest.slice(1);
  } else if (line.startsWith("\t")) {
    // Pure TAB domain header `\tD1 1` (v1.5 / v1.2 generic)
    isV12 = true;
    mode = "5w1h";
    tail = sub;
  } else {
    return null;
  }

  const sp = tail.indexOf(" ");
  if (sp === -1) {
    if (!isValidDomainId(tail)) return null;
    return { mode, id: tail, revision: 1, isV12 };
  }
  const idPart = tail.slice(0, sp);
  const revPart = tail.slice(sp + 1);
  if (!isValidDomainId(idPart) || !/^[1-9][0-9]*$/.test(revPart)) return null;
  return { mode, id: idPart, revision: Number(revPart), isV12 };
}

export function parseNemaStrict(source) {
  try {
    if (typeof source !== "string" || source.length === 0) {
      return { ok: false, error: "NEMA document cannot be empty" };
    }
    if (source.includes("\r")) {
      return { ok: false, error: "LF-only required in NEMA" };
    }
    if (!source.endsWith("\n\n\n") || source.endsWith("\n\n\n\n")) {
      return { ok: false, error: "NEMA must terminate with exactly two empty physical lines (three LF)" };
    }
    const body = source.slice(0, -3);
    if (!body) return { ok: false, error: "No content before termination" };
    if (body.includes("\n\n")) {
      return { ok: false, error: "Internal empty lines prohibited in NEMA" };
    }

    const lines = body.split("\n");
    let i = 0;
    const dictLines = [];
    let dictCount = 0;

    if (lines[0].startsWith("\t$") && /^[0-9]+$/.test(lines[0].slice(2))) {
      dictCount = Number(lines[0].slice(2));
      if (dictCount > 2048 || 1 + dictCount >= lines.length) {
        return { ok: false, error: "Invalid dictionary manifest size" };
      }
      dictLines.push(...lines.slice(0, 1 + dictCount));
      i = 1 + dictCount;
    }

    const domains = [];
    const seenDomainIds = new Set();
    let current = null;

    for (; i < lines.length; i++) {
      const line = lines[i];
      const hdr = parseDomainHeader(line);
      if (hdr) {
        if (current && current.records.length === 0 && !current.explicitHeader) {
          return { ok: false, error: `Domain '${current.id}' has no content` };
        }
        if (seenDomainIds.has(hdr.id)) {
          return { ok: false, error: `Duplicate domain ID '${hdr.id}'` };
        }
        seenDomainIds.add(hdr.id);
        current = {
          ...hdr,
          headerLines: [line],
          columns: hdr.mode === "5w1h" ? [...AXES_5W1H] : hdr.mode === "graph" ? [...COLUMNS_GRAPH] : [],
          explicitHeader: false,
          records: [],
        };
        domains.push(current);
        continue;
      }

      if (!current) {
        return { ok: false, error: `Line ${i + 1}: body before domain declaration` };
      }
      if (line.includes("\t") && !(current.isV12 && current.records.length === 0 && !current.explicitHeader)) {
        return { ok: false, error: `Line ${i + 1}: TAB character in body prohibited` };
      }

      // Check table width / header declarations right after domain header
      if (current.records.length === 0) {
        const wMatch = line.match(/^ ?\\%([1-9][0-9]*)$/);
        if (wMatch) {
          current.mode = "table";
          const nCols = Number(wMatch[1]);
          current.columns = Array.from({ length: nCols }, (_, k) => `column${k + 1}`);
          current.headerLines.push(line);
          continue;
        }
        if (!current.explicitHeader) {
          let hdrColsRaw = null;
          if (line.startsWith("\\# ")) hdrColsRaw = line.slice(3);
          else if (line.startsWith(" \\# ")) hdrColsRaw = line.slice(4);
          else if (line.startsWith("\t") && current.isV12) hdrColsRaw = line.slice(1);
          if (hdrColsRaw !== null) {
            const cols = splitUnescapedSpaces(hdrColsRaw);
            if (cols.length > 0 && new Set(cols).size === cols.length) {
              current.mode = "table";
              current.columns = cols;
              current.explicitHeader = true;
              current.headerLines.push(line);
              continue;
            }
          }
        }
      }

      // Continuation lines (`  ...` or single-space node/cont in Graph)
      if (line.startsWith(" ")) {
        const trimmedLead = line.replace(/^ +/, "");
        if (!trimmedLead) return { ok: false, error: `Line ${i + 1}: blank continuation` };
        if (current.mode === "graph" && !line.startsWith("  ")) {
          const toks = splitUnescapedSpaces(trimmedLead);
          if (toks.length >= 2 && isValidGraphIdent(toks[0]) && !toks[0].match(/[><=]/) && !current.records.some((r) => r.key === `node:${toks[0]}`)) {
            current.records.push({ key: `node:${toks[0]}`, rawLines: [line], nodeId: toks[0] });
            continue;
          }
        }
        if (current.records.length === 0) {
          return { ok: false, error: `Line ${i + 1}: continuation before first record` };
        }
        current.records[current.records.length - 1].rawLines.push(line);
        continue;
      }

      const cells = splitUnescapedSpaces(line);
      if (current.mode === "5w1h" && cells.length > 6) {
        return { ok: false, error: `Line ${i + 1}: 5W1H allows maximum 6 fields` };
      }
      if (current.mode === "table" && current.columns.length > 0 && cells.length > current.columns.length) {
        return { ok: false, error: `Line ${i + 1}: record exceeds table column width` };
      }

      let recKey = `exact:${line}`;
      let nodeId = null;
      let edgeSpec = null;
      if (current.mode === "graph") {
        const tok = cells[0];
        const em = tok.match(/^([A-Za-z][A-Za-z0-9_.:-]*)([><=])([A-Za-z][A-Za-z0-9_.:-]*)\2([A-Za-z][A-Za-z0-9_.:-]*)$/);
        if (tok.match(/[><=]/)) {
          if (!em) return { ok: false, error: `Line ${i + 1}: invalid graph edge expression '${tok}'` };
          edgeSpec = { src: em[1], id: em[3], dst: em[4], op: em[2] };
          recKey = `edge:${em[3]}`;
        } else {
          if (!isValidGraphIdent(tok)) return { ok: false, error: `Line ${i + 1}: invalid graph node ID '${tok}'` };
          nodeId = tok;
          recKey = `node:${tok}`;
        }
      }
      current.records.push({ key: recKey, rawLines: [line], cells, nodeId, edgeSpec });
    }

    if (domains.length === 0) return { ok: false, error: "No valid NEMA domains found" };

    for (const dom of domains) {
      if (dom.records.length === 0 && !(dom.mode === "table" && dom.columns.length > 0)) {
        return { ok: false, error: `Domain '${dom.id}' has no content` };
      }
      if (dom.mode === "graph") {
        const nodes = new Set();
        const edges = new Set();
        for (const r of dom.records) {
          if (r.nodeId) {
            if (nodes.has(r.nodeId)) return { ok: false, error: `Duplicate graph node '${r.nodeId}'` };
            nodes.add(r.nodeId);
          } else if (r.edgeSpec) {
            if (edges.has(r.edgeSpec.id)) return { ok: false, error: `Duplicate graph edge '${r.edgeSpec.id}'` };
            edges.add(r.edgeSpec.id);
          }
        }
        for (const r of dom.records) {
          if (r.edgeSpec && (!nodes.has(r.edgeSpec.src) || !nodes.has(r.edgeSpec.dst))) {
            return { ok: false, error: `Undefined graph node reference in edge '${r.edgeSpec.id}'` };
          }
        }
      }
      if (dom.mode === "table") {
        const firstCells = dom.records.map((r) => (r.cells && r.cells[0]) || "");
        if (firstCells.length > 0 && firstCells.every(Boolean) && new Set(firstCells).size === firstCells.length) {
          for (const r of dom.records) r.key = `row:${r.cells[0]}`;
        }
      }
    }

    return { ok: true, dictLines, domains };
  } catch (e) {
    return { ok: false, error: e.message || "Invalid NEMA syntax" };
  }
}

export function validateNemaSyntax(text) {
  return parseNemaStrict(text);
}

export function nemaSemanticMerge(baseText, oursText, theirsText) {
  const oursParsed = parseNemaStrict(oursText);
  if (!oursParsed.ok) return { clean: false, conflicts: 1, conflictHunks: [{ ours: oursParsed.error }], text: oursText };
  const theirsParsed = parseNemaStrict(theirsText);
  if (!theirsParsed.ok) return { clean: false, conflicts: 1, conflictHunks: [{ theirs: theirsParsed.error }], text: theirsText };

  const baseParsed = parseNemaStrict(baseText);
  if (!baseParsed.ok) {
    const fallback = threeWayMerge(baseText, oursText, theirsText);
    if (!fallback.clean) return fallback;
    const chk = parseNemaStrict(fallback.text);
    return chk.ok ? fallback : { clean: false, conflicts: 1, conflictHunks: [{ ours: chk.error }], text: fallback.text };
  }

  const conflicts = [];
  const dictOut =
    JSON.stringify(oursParsed.dictLines) === JSON.stringify(theirsParsed.dictLines) ||
    JSON.stringify(theirsParsed.dictLines) === JSON.stringify(baseParsed.dictLines)
      ? oursParsed.dictLines
      : JSON.stringify(oursParsed.dictLines) === JSON.stringify(baseParsed.dictLines)
      ? theirsParsed.dictLines
      : (conflicts.push({ base_line_start: 1, base_line_end: 1, ours: oursParsed.dictLines.join("\n"), theirs: theirsParsed.dictLines.join("\n") }), oursParsed.dictLines);

  const keyOf = (d) => `${d.mode}:${d.id}`;
  const bMap = new Map(baseParsed.domains.map((d) => [keyOf(d), d]));
  const oMap = new Map(oursParsed.domains.map((d) => [keyOf(d), d]));
  const tMap = new Map(theirsParsed.domains.map((d) => [keyOf(d), d]));
  const orderedKeys = [];
  const seenDom = new Set();
  for (const d of [...baseParsed.domains, ...oursParsed.domains, ...theirsParsed.domains]) {
    const k = keyOf(d);
    if (!seenDom.has(k)) {
      seenDom.add(k);
      orderedKeys.push(k);
    }
  }

  const outLines = [...dictOut];
  for (const k of orderedKeys) {
    const bd = bMap.get(k);
    const od = oMap.get(k);
    const td = tMap.get(k);
    const bh = bd ? bd.headerLines.join("\n") : null;
    const oh = od ? od.headerLines.join("\n") : null;
    const th = td ? td.headerLines.join("\n") : null;
    let hdrLines = null;
    if (bd && od && td) {
      hdrLines = oh === th || th === bh ? od.headerLines : oh === bh ? td.headerLines : (conflicts.push({ base_line_start: 1, base_line_end: 1, ours: oh, theirs: th }), od.headerLines);
    } else if (od && td) {
      hdrLines = oh === th ? od.headerLines : (conflicts.push({ base_line_start: 1, base_line_end: 1, ours: oh, theirs: th }), od.headerLines);
    } else if (od) hdrLines = od.headerLines;
    else if (td) hdrLines = td.headerLines;
    if (!hdrLines) continue;

    const bRecs = bd ? bd.records : [];
    const oRecs = od ? od.records : [];
    const tRecs = td ? td.records : [];
    const oRecMap = new Map(oRecs.map((r) => [r.key, r.rawLines.join("\n")]));
    const tRecMap = new Map(tRecs.map((r) => [r.key, r.rawLines.join("\n")]));
    const doneKeys = new Set();
    const mergedRecLines = [];

    for (const br of bRecs) {
      doneKeys.add(br.key);
      const bStr = br.rawLines.join("\n");
      const oStr = oRecMap.get(br.key);
      const tStr = tRecMap.get(br.key);
      if (oStr !== undefined && tStr !== undefined) {
        if (oStr === tStr || tStr === bStr) mergedRecLines.push(oStr);
        else if (oStr === bStr) mergedRecLines.push(tStr);
        else {
          conflicts.push({ base_line_start: 1, base_line_end: 1, ours: oStr, theirs: tStr });
          mergedRecLines.push(oStr);
        }
      } else if (oStr !== undefined && oStr !== bStr) {
        conflicts.push({ base_line_start: 1, base_line_end: 1, ours: oStr, theirs: "<deleted>" });
      } else if (tStr !== undefined && tStr !== bStr) {
        conflicts.push({ base_line_start: 1, base_line_end: 1, ours: "<deleted>", theirs: tStr });
      }
    }
    for (const or of oRecs) {
      if (doneKeys.has(or.key)) continue;
      doneKeys.add(or.key);
      const oStr = or.rawLines.join("\n");
      const tStr = tRecMap.get(or.key);
      if (tStr !== undefined && tStr !== oStr) {
        conflicts.push({ base_line_start: 1, base_line_end: 1, ours: oStr, theirs: tStr });
      }
      mergedRecLines.push(oStr);
    }
    for (const tr of tRecs) {
      if (!doneKeys.has(tr.key)) {
        doneKeys.add(tr.key);
        mergedRecLines.push(tr.rawLines.join("\n"));
      }
    }

    outLines.push(...hdrLines, ...mergedRecLines);
  }

  if (conflicts.length > 0) {
    const preview = threeWayMerge(baseText, oursText, theirsText).text;
    return { clean: false, conflicts: conflicts.length, conflictHunks: conflicts, text: preview };
  }

  const assembled = `${outLines.join("\n")}\n\n\n`;
  const finalCheck = parseNemaStrict(assembled);
  if (!finalCheck.ok) {
    return {
      clean: false,
      conflicts: 1,
      conflictHunks: [{ base_line_start: 1, base_line_end: 1, ours: finalCheck.error, theirs: "manual_resolution_required" }],
      text: threeWayMerge(baseText, oursText, theirsText).text,
    };
  }
  return { clean: true, conflicts: 0, conflictHunks: [], text: assembled };
}
