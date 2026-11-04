# Vendor bundle licence & runtime review — gemarc-5d-atlas-v10.0

| | |
|---|---|
| Generated at | 2026-10-22T02:51:42Z (see `LICENSES.json` → `generatedAt`) |
| Source tree | `D:\GitGit\docs\gemarc-5d-atlas-v10.0\.tooling\node_modules` |
| Dependency tree source | `npm ls --all --json` (run in `.tooling/`), reconciled against the installed `node_modules` on disk |
| Licence field source | Each package's own `package.json` → `license`, read verbatim (no legal interpretation, no SPDX normalisation) |
| Checksums | `node:crypto` sha256 over raw file bytes |
| Runtime verification | System Google Chrome via Playwright `chromium.launch({ channel: 'chrome' })`, page loaded over `file://` |

---

## 1. Licence verdict — **all permissive, no manual review required**

> **All 8 packages in the shipped runtime closure are MIT. There is nothing requiring human licence review.**

`reviewRequired` is computed mechanically: `true` only when the licence string is not one of
`MIT / ISC / Apache-2.0 / BSD-2-Clause / BSD-3-Clause / 0BSD / Unlicense / CC0-1.0`.

### 1.1 Shipped runtime closure (8 packages — these are inlined into the bundles)

| # | Package | Version | Licence | `licenseFile` (relative to `vendor/`) | `reviewRequired` |
|---|---|---|---|---|---|
| 1 | `dagre` | 0.8.5 | MIT | `../.tooling/node_modules/dagre/LICENSE` | false |
| 2 | `events` | 3.3.0 | MIT | `../.tooling/node_modules/events/LICENSE` | false |
| 3 | `graphlib` | 2.1.8 | MIT | `../.tooling/node_modules/graphlib/LICENSE` | false |
| 4 | `graphology` | 0.26.0 | MIT | `../.tooling/node_modules/graphology/LICENSE.txt` | false |
| 5 | `graphology-types` | 0.24.8 | MIT | `../.tooling/node_modules/graphology-types/LICENSE.txt` | false |
| 6 | `konva` | 10.9.0 | MIT | `../.tooling/node_modules/konva/LICENSE` | false |
| 7 | `lodash` | 4.18.1 | MIT | `../.tooling/node_modules/lodash/LICENSE` | false |
| 8 | `three` | 0.186.1 | MIT | `../.tooling/node_modules/three/LICENSE` | false |

**Licence distribution: `{ "MIT": 8 }` — `reviewRequired: true` count = 0.**

Note on `graphology-types`: it is a types-only package (zero runtime code) that enters the tree as an
*installed peer dependency* of `graphology`. It is listed in `LICENSES.json` for tree completeness but
contributes no bytes to any bundle. It is therefore recorded separately as
`typesOnlyPackagesInClosure` on the `graph.bundle.js` entry rather than in `sourcePackages`.

Note on the identical `dagre` / `graphlib` licence-file SHA-256
(`6a349742a6cb219d5a2fc8d0844f6d89a6efc62e20c664450d884fc7ff2d6015`): the two `LICENSE` files are
**byte-identical by authorship**, not by accident of copying — both carry
`Copyright (c) 2012-2014 Chris Pettitt` followed by the same MIT text. Verified by
`Get-FileHash -Algorithm SHA256` on both files and by reading their first 12 lines.

### 1.2 Build/test toolchain (54 packages — NOT shipped in `vendor/`)

These live in `.tooling/` and are **not** part of any deliverable; they are recorded so the audit
covers the whole installed tree.

| Licence | Count |
|---|---|
| MIT | 48 |
| Apache-2.0 | 3 (`playwright`, `playwright-core`, `expect-type`) |
| ISC | 2 (`picocolors`, `siginfo`) |
| BSD-3-Clause | 1 (`source-map-js`) |

`reviewRequired: true` count in the toolchain = **0** (Apache-2.0 / ISC / BSD-3-Clause are all on the
permissive allow-list). Full per-package detail is in `LICENSES.json` → `toolchainPackages`.

---

## 2. Runtime compatibility verification

Spec basis: `portable_html_delivery.forbidden_at_runtime` — no fetch of local JSON, no `file://` ES
module import, no mandatory dynamic Worker file load, no remote fonts/CDN, no arbitrary `eval`, no
external service required to start.

All three bundles are **IIFE-wrapped, classic scripts** with a single global export each, so
`<script src="...">` works directly over `file://`:

| Bundle | Bytes | SHA-256 | First 60 chars | Global export |
|---|---|---|---|---|
| `three.bundle.js` | 743322 | `5349d20e75182856df40864110ad8612d0237f99cdb02077fc14d0be399746b5` | `var THREE=(()=>{var Jh=Object.defineProperty;var gg=Object.getOwn…` | `var THREE=` @ offset 0 |
| `konva.bundle.js` | 194210 | `43a53d79571682e6785538917489ab56ddc5241ea5a77428f0fdb03c42205e93` | `var Konva=(()=>{var ke=Object.defineProperty;var Yn=Object.getOwnProp…` | `var Konva=` @ offset 0, re-export `var Konva = Konva.Konva;` at EOF |
| `graph.bundle.js` | 154475 | `004fbe9919478e8e162faf55fab9bc671f438b7fe5231302565ed14ddb7a53af` | `var Graphology=(()=>{var pg=Object.create;var At=Object.defineProperty;…` | `var Graphology=` @ offset 0, `var Dagre = Graphology.dagre;` @ offset 154445 |

Wrapper form: arrow-IIFE `var X=(()=>{…})();` in all three. `"use strict"` literal present in
`graph.bundle.js` only (not in `three.bundle.js` / `konva.bundle.js`).

### 2.1 Residue scan — method and real output

Two independent passes were run over each bundle:

* **AST pass** — `acorn` 8.19.0 parsed each bundle as `sourceType:'script'`, then the AST was walked
  to count real syntax nodes. This pass *cannot* produce false positives from strings or regexes.
* **Raw textual pass** — regex scan of the raw bytes, to expose the false positives that the AST
  pass eliminates.

**AST pass — actual output:**

```
############ three.bundle.js ############
COUNTS {"importDecl":0,"exportNamed":0,"exportDefault":0,"exportAll":0,"importMeta":0,
        "requireCall":0,"requireIdent":0,"dynamicImport":0,"fetchCall":2,"fetchMember":0,
        "xhrIdent":0,"webSocketIdent":0,"evalCall":0,"evalMember":0,"newFunction":0,
        "functionCall":0,"workerNew":0,"importScripts":0,"scriptCreateElement":0}

############ konva.bundle.js ############
COUNTS {"importDecl":0,"exportNamed":0,"exportDefault":0,"exportAll":0,"importMeta":0,
        "requireCall":0,"requireIdent":0,"dynamicImport":0,"fetchCall":0,"fetchMember":0,
        "xhrIdent":0,"webSocketIdent":0,"evalCall":0,"evalMember":0,"newFunction":0,
        "functionCall":0,"workerNew":0,"importScripts":0,"scriptCreateElement":0}

############ graph.bundle.js ############
COUNTS {"importDecl":0,"exportNamed":0,"exportDefault":0,"exportAll":0,"importMeta":0,
        "requireCall":0,"requireIdent":8,"dynamicImport":0,"fetchCall":0,"fetchMember":0,
        "xhrIdent":0,"webSocketIdent":0,"evalCall":0,"evalMember":0,"newFunction":0,
        "functionCall":1,"workerNew":0,"importScripts":0,"scriptCreateElement":0}
```

**Raw textual pass — actual output (false positives made explicit):**

| Pattern | three | konva | graph |
|---|---|---|---|
| `import` statement | 0 | 0 | 0 |
| `export` statement | 0 | 0 | 0 |
| `require(` | 0 | 0 | **1** |
| `import(` | 0 | 0 | **2** |
| `fetch(` | **3** | 0 | 0 |
| `XMLHttpRequest` | 0 | 0 | 0 |
| `WebSocket` | 0 | 0 | 0 |
| `eval(` | 0 | 0 | 0 |
| `new Function(` | 0 | 0 | 0 |
| `Function(` | 0 | 0 | 1 |
| `new Worker(` / `SharedWorker` | 0 | 0 | 0 |
| `importScripts(` | 0 | 0 | 0 |
| `createElement('script')` | 0 | 0 | 0 |
| `sendBeacon` | 0 | 0 | 0 |
| `sourceMappingURL` | 0 | 0 | 0 |
| `import.meta` | 0 | 0 | 0 |

**Every raw-vs-AST delta was individually classified (AST range intersection against comments /
string literals / template literals / regex literals):**

| Bundle | Raw hit | Class | Real residue? |
|---|---|---|---|
| three | `fetch(` @ L5 | **string-literal** — the warning text `"ImageBitmapLoader: fetch() not supported."` | No — false positive |
| three | `fetch(` @ L5 | EXECUTABLE CODE — `fetch(a).then(c=>{…` in `FileLoader.load()` | **Yes — real, but inert unless called** |
| three | `fetch(` @ L5 | EXECUTABLE CODE — `let l=fetch(t,o).then(function(c){return c.blob()})` in `ImageBitmapLoader.load()` | **Yes — real, but inert unless called** |
| graph | `require(` @ L1 | EXECUTABLE CODE — `ut.require&&ut.require("util").types` (lodash node-util detection) | **Yes — guarded + wrapped in try/catch, never reached in a browser** |
| graph | `import(` @ L1 | EXECUTABLE CODE — graphology's **public method** `import(t,r=!1){…}` on the Graph class | No — a method definition/call, **not** a dynamic `import()` expression |
| graph | `import(` @ L1 | EXECUTABLE CODE — call site `i.import(t)` on a Graph instance | No — same method, not dynamic import |
| graph | `Function(` @ L1 | EXECUTABLE CODE — lodash global-object detection `rv\|\|nv\|\|Function("return this")()` | **Yes — CSP `unsafe-eval` capable, but not executed in a browser** |

### 2.2 The three non-zero findings, in full

1. **`three.bundle.js` — 2 real `fetch()` call sites** (`FileLoader.load`, `ImageBitmapLoader.load`).
   They are *latent*: nothing in the bundle invokes them at load time, and `_smoke.html` never calls
   them (observed fetch count = 0, §3). They only fire if application code explicitly calls
   `new THREE.FileLoader().load(url)` / `new THREE.ImageBitmapLoader().load(url)`. **This is a
   capability, not an active violation** — but the consuming app must not use those two loaders
   under `portable_html_delivery`, or the delivery becomes non-portable. This is disclosed on the
   smoke page itself (functional probe #5) so it cannot be missed.
2. **`graph.bundle.js` — 8 bare `require` identifiers** from the esbuild CommonJS interop shim:
   `typeof require!="undefined"?require:typeof Proxy!="undefined"?new Proxy(e,{get:(t,r)=>(typeof require!="undefined"?require:t)[r]}):e`
   and `if(typeof require!="undefined")return require.apply(this,arguments);throw Error('Dynamic require of "'+e+'" is not supported')`.
   In a browser `typeof require === "undefined"`, so the shim resolves to the `Proxy` branch and the
   `__require` path throws the explicit *"Dynamic require of … is not supported"* error **only if** a
   module is actually requested at runtime. The bundle loads cleanly (verified: 0 page errors), so no
   `__require` is triggered during initialisation.
3. **`graph.bundle.js` — 1 `Function("return this")()`** (lodash's `_root` / global-object detection).
   This is the only `unsafe-eval`-shaped construct in the whole vendor set. It is **short-circuited in
   a browser** by the preceding `nv = typeof self=="object"&&self&&self.Object===Object&&self` term,
   and the instrumented probe in `_smoke.html` recorded **0 `Function(...)` calls** at load time (§3),
   which proves empirically that it is not executed under `file://` in Chrome. It remains a latent
   `unsafe-eval` dependency *if* `self` were ever falsy — recorded here so the risk is documented
   rather than hidden.

### 2.3 Remote-loading residue — every `http(s)` occurrence, classified

Total `http://` / `https://` literal occurrences across the three bundles: **5**.

| Bundle | Line | Class | Content | Executable? |
|---|---|---|---|---|
| `three.bundle.js` | 1 | string-literal | `document.createElementNS("http://www.w3.org/1999/xhtml",s)` | No — XML **namespace URI**, never dereferenced as a URL |
| `three.bundle.js` | 3856 | template-literal | `// https://jcgt.org/published/0007/04/01/` — a citation comment inside the embedded GLSL shader source string | No — inside a shader-source string |
| `konva.bundle.js` | 14 | string-literal | error message: `"Unable to get data URL. … For more info read https://konvajs.org/docs/posts/Tainted_Canvas.html."` | No — error message text |
| `konva.bundle.js` | 14 | string-literal | error message: `"Unable to apply filter. … This post my help you https://konvajs.org/docs/posts/Tainted_Canvas.html."` | No — error message text |
| `konva.bundle.js` | 15 | template-literal | `<svg xmlns="http://www.w3.org/2000/svg" …>` (Konva arrow-head icon) | No — SVG **namespace URI** |
| `graph.bundle.js` | — | — | **no `http(s)` occurrence at all** | — |

**No occurrence is a network fetch target.** No remote font, CDN, or cross-origin asset reference
exists in any bundle. There is no `sourceMappingURL` comment in any bundle.

### 2.4 Other forbidden-at-runtime mechanisms

| Mechanism | three | konva | graph |
|---|---|---|---|
| `file://` ES module import (`import` stmt / `import.meta` / dynamic `import()`) | 0 | 0 | 0 |
| `require()` **call** | 0 | 0 | 0 (8 guarded *identifiers*, §2.2-2) |
| `new Worker()` / `new SharedWorker()` | 0 | 0 | 0 |
| `importScripts()` | 0 | 0 | 0 |
| `document.createElement('script')` | 0 | 0 | 0 |
| `XMLHttpRequest` | 0 | 0 | 0 |
| `WebSocket` | 0 | 0 | 0 |
| `eval()` | 0 | 0 | 0 |
| `new Function()` | 0 | 0 | 0 (1 `Function(...)` call, §2.2-3) |
| `navigator.sendBeacon` | 0 | 0 | 0 |

---

## 3. Real browser run — `_smoke.html` in system Chrome over `file://`

`vendor/_smoke.html` was rewritten so that the page **computes and displays its own results**: an
instrumentation block is installed **before** the three `<script src>` bundles, wrapping
`fetch`, `Function`, `eval`, `XMLHttpRequest`, `WebSocket` and `Worker` so that any load-time call is
counted and displayed. The page then renders the assertion tables, the verdict banner, the
environment block and a plain-text log — no placeholder strings.

Command (run from `.tooling/`, using the already-installed `playwright` package, **no browser
download** — `channel: 'chrome'` uses the system Google Chrome):

```
node --input-type=module -   # inline ESM script, cwd = .tooling
  chromium.launch({ channel: 'chrome' })
  page.goto('file:///D:/GitGit/docs/gemarc-5d-atlas-v10.0/vendor/_smoke.html', { waitUntil: 'load' })
```

Browser actually used: **Google Chrome 155.0.8059.85** —
`browser.version()` returned `155.0.8059.85`, which matches the system install at
`C:\Program Files\Google\Chrome\Application\chrome.exe`
(`ProductName = "Google Chrome"`, `ProductVersion = 155.0.8059.85`). Launched in headless mode
(`HeadlessChrome/155.0.0.0` in the UA string). No browser binary was downloaded.

**Actual output:**

```
===== document.title =====
SMOKE_PASS THREE=186 Konva=10.9.0 Graphology=present Dagre=present

===== VERDICT =====
SMOKE_PASS — 5/5 global assertions, 5/5 functional probes, 6/6 residue probes, 0 remote requests

===== 1. GLOBAL API ASSERTIONS =====
  1  |  typeof THREE.REVISION  |  string (non-empty)  |  string ("186")  |  PASS
  2  |  typeof THREE.WebGLRenderer  |  function  |  function  |  PASS
  3  |  typeof Konva.Stage  |  function  |  function  |  PASS
  4  |  typeof Graphology.Graph  |  function  |  function  |  PASS
  5  |  typeof Dagre.graphlib  |  object  |  object  |  PASS

===== 2. FUNCTIONAL PROBES =====
  1  |  THREE: new THREE.Scene() + PerspectiveCamera + add  |  ok, scene.children=1, REVISION=186  |  PASS
  2  |  Konva: new Konva.Stage(container) + Layer + Rect  |  ok, layers=1, version=10.9.0  |  PASS
  3  |  Graphology: new Graphology.Graph() + directed edge  |  ok, order=2, size=1, hasDirectedEdge=true  |  PASS
  4  |  Dagre: graphlib.Graph + dagre.layout()  |  ok, nodes=2, edge(a,b)={"points":[{"x":50,"y":15},{"x":75,"y":15},{"x":100,"y":15}]}  |  PASS
  5  |  Disclosed loader API surface (present but NOT invoked): THREE.FileLoader / THREE.ImageBitmapLoader  |  FileLoader=function, ImageBitmapLoader=function (these contain the 2 fetch() call sites; not called by this page)  |  PASS

===== 3. RUNTIME RESIDUE PROBES =====
  1  |  window.fetch(...)  |  0  |  PASS
  2  |  new XMLHttpRequest()  |  0  |  PASS
  3  |  new WebSocket(...)  |  0  |  PASS
  4  |  new Worker(...)  |  0  |  PASS
  5  |  eval(...)  |  0  |  PASS
  6  |  Function(...)  [CSP unsafe-eval capable]  |  0  |  PASS

===== 4. ENVIRONMENT =====
location.href        = file:///D:/GitGit/docs/gemarc-5d-atlas-v10.0/vendor/_smoke.html
protocol             = file:
userAgent            = Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) HeadlessChrome/155.0.0.0 Safari/537.36
script[src] elements = ["three.bundle.js","konva.bundle.js","graph.bundle.js"]
performance resources = []
remote resources      = [] (must be empty)
probe runtime errors  = []

===== 5. PLAIN LOG =====
typeof THREE.REVISION = string ("186")  [PASS]
typeof THREE.WebGLRenderer = function  [PASS]
typeof Konva.Stage = function  [PASS]
typeof Graphology.Graph = function  [PASS]
typeof Dagre.graphlib = object  [PASS]
THREE: new THREE.Scene() + PerspectiveCamera + add = ok, scene.children=1, REVISION=186  [PASS]
Konva: new Konva.Stage(container) + Layer + Rect = ok, layers=1, version=10.9.0  [PASS]
Graphology: new Graphology.Graph() + directed edge = ok, order=2, size=1, hasDirectedEdge=true  [PASS]
Dagre: graphlib.Graph + dagre.layout() = ok, nodes=2, edge(a,b)={"points":[{"x":50,"y":15},{"x":75,"y":15},{"x":100,"y":15}]}  [PASS]
Disclosed loader API surface (present but NOT invoked): THREE.FileLoader / THREE.ImageBitmapLoader = FileLoader=function, ImageBitmapLoader=function (these contain the 2 fetch() call sites; not called by this page)  [PASS]
window.fetch(...) calls at load = 0  [PASS]
new XMLHttpRequest() calls at load = 0  [PASS]
new WebSocket(...) calls at load = 0  [PASS]
new Worker(...) calls at load = 0  [PASS]
eval(...) calls at load = 0  [PASS]
Function(...)  [CSP unsafe-eval capable] calls at load = 0  [PASS]

===== NETWORK REQUESTS OBSERVED (4) =====
  file:///D:/GitGit/docs/gemarc-5d-atlas-v10.0/vendor/_smoke.html
  file:///D:/GitGit/docs/gemarc-5d-atlas-v10.0/vendor/three.bundle.js
  file:///D:/GitGit/docs/gemarc-5d-atlas-v10.0/vendor/konva.bundle.js
  file:///D:/GitGit/docs/gemarc-5d-atlas-v10.0/vendor/graph.bundle.js

===== CONSOLE MESSAGES (0) =====
  (none)

===== PAGE ERRORS (0) =====
  (none)

===== RESULT: PASS =====
```

**Console errors: 0. Page errors: 0. Remote (`http`/`https`) requests: 0.**
The only 4 network requests are `file://` reads of the page itself and the three local bundles.

The `performance resources = []` line is expected: `file://` sub-resource loads are not recorded in
the Resource Timing API. The Playwright request log above is the authoritative network evidence.

---

## 4. Checksums

`SHA256SUMS.txt` (standard `sha256sum` format, `<64-hex><two spaces><filename>`, computed with
`node:crypto`) covers all six delivered files in `vendor/`: the three bundles, `LICENSES.json`,
`LICENSES-REVIEW.md` and `_smoke.html`. It cannot contain its own hash and is therefore not listed
inside itself. Verify with:

```
node -e "const fs=require('fs'),c=require('crypto');for(const l of fs.readFileSync('vendor/SHA256SUMS.txt','utf8').trim().split('\n')){const[h,f]=l.split(/\s{2}/);const a=c.createHash('sha256').update(fs.readFileSync('vendor/'+f)).digest('hex');console.log(a===h?'OK  ':'FAIL',f,a===h?'':'expected '+h+' got '+a);}"
```

---

## 5. NOT_TESTED (explicitly not verified — do not read as "should be fine")

| Item | Status |
|---|---|
| Constructing a real `THREE.WebGLRenderer` (WebGL context acquisition) | **NOT_TESTED** — only `typeof THREE.WebGLRenderer === 'function'` was asserted. The Chrome run is headless; no GPU/WebGL context was requested. |
| Firefox / Safari / Edge behaviour | **NOT_TESTED** — only system Google Chrome 155.0.8059.85 (headless, via Playwright `channel: 'chrome'`) was run. |
| The real v10.0 application (`index.html`, `src/`) consuming these bundles | **NOT_TESTED** — the verification target was `vendor/_smoke.html` only, per the assigned scope. `src/`, `models/`, `index.html`, `dist/` and `tests/` were not modified and were not exercised. |
| Behaviour under a strict `Content-Security-Policy` (e.g. `script-src 'self'`, no `unsafe-eval`) | **NOT_TESTED** — no CSP header was served; the `eval`/`Function` call counters were instrumented instead. A CSP without `unsafe-eval` is expected to work given `Function("return this")` was not executed, but this is inference, not a measurement. |
| `THREE.FileLoader` / `THREE.ImageBitmapLoader` actually fetching a URL | **NOT_TESTED and deliberately not exercised** — the fetch call sites exist (§2.2-1) but were never invoked; the smoke page asserts only that the loaders are *present*. |
| Any build/reproducibility check of the bundles themselves | **NOT_TESTED** — `.tooling/build.mjs` is absent from the tooling directory, so the bundles could not be regenerated and diffed against their source. The bundles were audited as-shipped, not rebuilt. |
| Long-running / memory behaviour, WebGL render output, animation loop | **NOT_TESTED** — out of scope for a load-and-assert smoke test. |
