// syntax.js — IDE1.0 IDE Shell 语言识别 + 语法高亮
// SPDX-License-Identifier: MIT
//
// 14+ 语言 regex lexer. 每个语言定义 keywords + 注释 + 字符串 + 数字等 token 类型.
// highlightLine() 把一行文本切成 token 数组, renderVim 用 token class 套 HTML <span>.
//
// 设计: 纯 regex 无依赖. 万行级别不卡 (按需增量 highlight). 所有语言共享相同 API.

"use strict";

(function (global) {
  // ===== 语言识别 (扩展名 → id) =====
  const EXT_MAP = {
    "rs": "rust", "py": "python", "pyi": "python", "pyx": "python",
    "js": "javascript", "mjs": "javascript", "cjs": "javascript",
    "ts": "typescript", "tsx": "typescript", "jsx": "javascript",
    "go": "go", "c": "c", "h": "c", "cpp": "cpp", "cxx": "cpp", "hpp": "cpp", "hxx": "cpp", "cc": "cpp",
    "java": "java", "rb": "ruby", "php": "php", "cs": "csharp",
    "swift": "swift", "kt": "kotlin", "m": "objc", "mm": "objc",
    "sh": "bash", "bash": "bash", "zsh": "bash", "fish": "bash",
    "html": "html", "htm": "html", "xml": "xml",
    "css": "css", "scss": "css", "less": "css",
    "json": "json", "jsonc": "json", "json5": "json",
    "md": "markdown", "markdown": "markdown",
    "yaml": "yaml", "yml": "yaml",
    "toml": "toml", "ini": "ini", "cfg": "ini",
    "sql": "sql", "Dockerfile": "dockerfile",
    "diff": "diff", "patch": "diff",
    "vim": "vim",
    "lua": "lua", "pl": "perl", "r": "r",
    "hs": "haskell", "ex": "elixir", "exs": "elixir", "erl": "erlang",
    "scala": "scala", "clj": "clojure", "dart": "dart",
    "txt": "plain", "text": "plain", "log": "plain",
  };

  // 文件名 → 语言 (用于 Dockerfile / Makefile 等无扩展名)
  const NAME_MAP = {
    "Dockerfile": "dockerfile", "Makefile": "makefile",
    "CMakeLists.txt": "cmake", ".bashrc": "bash", ".zshrc": "bash",
    ".gitignore": "plain", ".gitattributes": "plain", "Cargo.toml": "toml",
  };

  function detectLanguage(filePath) {
    if (!filePath) return "plain";
    const name = filePath.split(/[\\/]/).pop() || "";
    if (NAME_MAP[name]) return NAME_MAP[name];
    const dot = name.lastIndexOf(".");
    const ext = dot >= 0 ? name.slice(dot + 1).toLowerCase() : "";
    return EXT_MAP[ext] || "plain";
  }

  function languageLabel(lang) {
    const labels = {
      rust: "Rust", python: "Python", javascript: "JavaScript", typescript: "TypeScript",
      go: "Go", c: "C", cpp: "C++", java: "Java", ruby: "Ruby", php: "PHP",
      csharp: "C#", swift: "Swift", kotlin: "Kotlin", objc: "Objective-C",
      bash: "Shell", html: "HTML", xml: "XML", css: "CSS", json: "JSON",
      markdown: "Markdown", yaml: "YAML", toml: "TOML", ini: "INI",
      sql: "SQL", dockerfile: "Dockerfile", diff: "Diff", vim: "Vim Script",
      lua: "Lua", perl: "Perl", r: "R", haskell: "Haskell", elixir: "Elixir",
      erlang: "Erlang", scala: "Scala", clojure: "Clojure", dart: "Dart",
      makefile: "Makefile", cmake: "CMake", plain: "Plain",
    };
    return labels[lang] || lang;
  }

  // ===== 通用 token 模式 (大部分语言共享) =====
  // 数字: 整数 + 浮点 + 十六进制 + 八进制 + 后缀
  const RE_NUMBER = String.raw`\b(?:0[xX][0-9a-fA-F]+(?:_[0-9a-fA-F]+)*|0[oO][0-7]+(?:_[0-7]+)*|0[bB][01]+(?:_[01]+)*|\d+(?:_\d+)*(?:\.\d+(?:_\d+)*)?(?:[eE][+-]?\d+)?[fFlLuU]*)\b`;
  // 标识符 (普通单词)
  const RE_IDENT = String.raw`[A-Za-z_][A-Za-z0-9_]*`;

  // ===== 逐语言定义 =====
  // 每个 lexer = { lineComment, blockComment, strings: [{open, close, escape}], keywords: Set, types?: Set,
  //                extraRules?: [{pattern, cls}], numbers?: bool }
  const LANGS = {
    rust: {
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: 'r#"', close: '"#', escape: null, raw: true }],
      keywords: new Set(["as","async","await","break","const","continue","crate","dyn","else","enum","extern","false","fn","for","if","impl","in","let","loop","match","mod","move","mut","pub","ref","return","self","Self","static","struct","super","trait","true","type","unsafe","use","where","while","box","do","final","macro","offsetof","override","priv","proc","pure","typeof","unsized","virtual","yield","try","union"]),
      types: new Set(["bool","char","str","String","u8","u16","u32","u64","u128","usize","i8","i16","i32","i64","i128","isize","f32","f64","Vec","Option","Result","Box","Rc","Arc","HashMap","BTreeMap","HashSet","BTreeSet"]),
      extraRules: [
        { pattern: String.raw`\b(?:0x[0-9a-fA-F_]+|0o[0-7_]+|0b[01_]+|\d[\d_]*(?:\.\d[\d_]*)?(?:[eE][+-]?\d+)?[fFuUiI]*(?:_?(?:usize|isize|u8|u16|u32|u64|u128|i8|i16|i32|i64|i128|f32|f64))?)\b`, cls: "num" },
        { pattern: String.raw`@\w+`, cls: "attr" }, // 属性宏
        { pattern: String.raw`#!?\[[^\]]*\]`, cls: "attr" },
        { pattern: String.raw`\b(?:true|false|None|Some|Ok|Err)\b`, cls: "bool" },
        { pattern: String.raw`'[^\n\\']'`, cls: "char" },
        { pattern: String.raw`(?:->|=>|\.\.|\.\.<|\?|<|>|=)`, cls: "op" },
      ],
    },

    python: {
      lineComment: "#",
      blockComment: ['"""', '"""'],  // 简化: 只识别 triple-quoted
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }, { open: '"""', close: '"""', escape: "\\" }, { open: "'''", close: "'''", escape: "\\" }, { open: 'f"', close: '"', escape: "\\", fstring: true }, { open: "f'", close: "'", escape: "\\", fstring: true }],
      keywords: new Set(["False","None","True","and","as","assert","async","await","break","class","continue","def","del","elif","else","except","finally","for","from","global","if","import","in","is","lambda","nonlocal","not","or","pass","raise","return","try","while","with","yield","match","case","self"]),
      types: new Set(["int","float","complex","str","bytes","bool","list","dict","set","tuple","frozenset","object","type","None","Any","Optional","Union","Callable","Iterable","Iterator","Generator","TypeVar","Generic"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`@\w+`, cls: "decorator" },
        { pattern: String.raw`__(?:\w+)__`, cls: "special" },
        { pattern: String.raw`\{[^{}]*\}`, cls: "fmt", within: "fstring" }, // f-string {}
      ],
    },

    javascript: {
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }, { open: '`', close: '`', escape: "\\", template: true }],
      keywords: new Set(["abstract","as","async","await","break","case","catch","class","const","continue","debugger","default","delete","do","else","enum","export","extends","false","finally","for","from","function","if","implements","import","in","instanceof","interface","let","new","null","of","package","private","protected","public","return","static","super","switch","this","throw","true","try","typeof","var","void","while","with","yield","async"]),
      types: new Set(["any","boolean","number","string","object","never","unknown","symbol","bigint","Array","Promise","Map","Set","WeakMap","WeakSet","Date","RegExp","Error"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`\b(?:true|false|null|undefined|NaN|Infinity)\b`, cls: "bool" },
        { pattern: String.raw`\$\{[^}]*\}`, cls: "fmt", within: "template" },
      ],
    },

    typescript: {
      // 与 javascript 类似但加类型关键字
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }, { open: '`', close: '`', escape: "\\", template: true }],
      keywords: new Set(["abstract","as","async","await","break","case","catch","class","const","continue","debugger","default","delete","do","else","enum","export","extends","false","finally","for","from","function","if","implements","import","in","instanceof","interface","let","new","null","of","package","private","protected","public","readonly","return","static","super","switch","this","throw","true","try","typeof","var","void","while","with","yield"]),
      types: new Set(["any","boolean","number","string","object","never","unknown","symbol","bigint","Array","Promise","Map","Set","WeakMap","WeakSet","Date","RegExp","Error","Partial","Required","Readonly","Record","Pick","Omit","Exclude","Extract","ReturnType"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`\b(?:true|false|null|undefined|NaN|Infinity)\b`, cls: "bool" },
        { pattern: String.raw`\$\{[^}]*\}`, cls: "fmt", within: "template" },
      ],
    },

    go: {
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: '`', close: '`', escape: null, raw: true }],
      keywords: new Set(["break","case","chan","const","continue","default","defer","else","fallthrough","for","func","go","goto","if","import","interface","map","package","range","return","select","struct","switch","type","var","true","false","nil"]),
      types: new Set(["bool","byte","complex64","complex128","error","float32","float64","int","int8","int16","int32","int64","rune","string","uint","uint8","uint16","uint32","uint64","uintptr"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`\b(?:true|false|nil)\b`, cls: "bool" },
      ],
    },

    c: {
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }],
      keywords: new Set(["auto","break","case","char","const","continue","default","do","double","else","enum","extern","float","for","goto","if","inline","int","long","register","restrict","return","short","signed","sizeof","static","struct","switch","typedef","union","unsigned","void","volatile","while","_Bool","_Complex","_Imaginary","_Atomic","_Generic","_Noreturn","_Static_assert","_Thread_local"]),
      types: new Set(["bool","size_t","ssize_t","ptrdiff_t","intptr_t","uintptr_t","FILE","NULL","stdin","stdout","stderr"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`#\s*\w+`, cls: "preproc" }, // 预编译指令
        { pattern: String.raw`'[^\n\\']'`, cls: "char" },
      ],
    },

    cpp: {
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }],
      keywords: new Set(["auto","break","case","catch","char","class","const","constexpr","continue","default","delete","do","double","dynamic_cast","else","enum","explicit","export","extern","false","float","for","friend","goto","if","inline","int","long","mutable","namespace","new","noexcept","nullptr","operator","private","protected","public","register","reinterpret_cast","return","short","signed","sizeof","static","static_cast","struct","switch","template","this","throw","true","try","typedef","typeid","typename","union","unsigned","using","virtual","void","volatile","while","and","or","not","xor"]),
      types: new Set(["bool","size_t","string","String","vector","map","unordered_map","set","unordered_set","pair","tuple","shared_ptr","unique_ptr","weak_ptr"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`#\s*\w+`, cls: "preproc" },
        { pattern: String.raw`::`, cls: "op" },
      ],
    },

    java: {
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }],
      keywords: new Set(["abstract","assert","boolean","break","byte","case","catch","char","class","const","continue","default","do","double","else","enum","extends","final","finally","float","for","goto","if","implements","import","instanceof","int","interface","long","native","new","package","private","protected","public","return","short","static","strictfp","super","switch","synchronized","this","throw","throws","transient","try","void","volatile","while","yield","var","record","sealed","permits","non-sealed"]),
      types: new Set(["String","Object","Boolean","Byte","Character","Double","Float","Integer","Long","Short","Number","Throwable","Exception","RuntimeException","List","ArrayList","Map","HashMap","Set","HashSet","Optional","Stream"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`@[A-Za-z_]\w*`, cls: "annot" },
      ],
    },

    csharp: {
      lineComment: "//",
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: '@"', close: '"', escape: '""', raw: true }, { open: "'", close: "'", escape: "\\" }],
      keywords: new Set(["abstract","as","base","break","case","catch","checked","class","const","continue","default","delegate","do","else","enum","event","explicit","extern","false","finally","fixed","for","foreach","goto","if","implicit","in","interface","internal","is","lock","namespace","new","null","object","operator","out","override","params","private","protected","public","readonly","ref","return","sealed","sizeof","stackalloc","static","struct","switch","this","throw","true","try","typeof","unchecked","unsafe","using","var","virtual","void","volatile","while","yield","async","await","nameof"]),
      types: new Set(["bool","byte","char","decimal","double","float","int","long","sbyte","short","string","uint","ulong","ushort","object","dynamic","String","Int32","List","Dictionary","IEnumerable","Task"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`\$\{[^}]*\}`, cls: "fmt", within: "string" },
      ],
    },

    ruby: {
      lineComment: "#",
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }],
      keywords: new Set(["BEGIN","END","alias","and","begin","break","case","class","def","defined?","do","else","elsif","end","ensure","false","for","if","in","module","next","nil","not","or","redo","rescue","retry","return","self","super","then","true","undef","unless","until","when","while","yield"]),
      types: new Set(["Array","Hash","String","Symbol","Integer","Float","Numeric","Object","Class","Proc","Lambda","IO","File"]),
      extraRules: [
        { pattern: String.raw`:\w+`, cls: "symbol" },
        { pattern: String.raw`@[A-Za-z_]\w*`, cls: "ivar" },
        { pattern: String.raw`@@[A-Za-z_]\w*`, cls: "cvar" },
        { pattern: String.raw`%[a-zA-Z][^\n]*?[a-zA-Z]`, cls: "string" },
      ],
    },

    bash: {
      lineComment: "#",
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: null, raw: true }],
      keywords: new Set(["if","then","else","elif","fi","case","esac","for","select","while","until","do","done","function","return","in","break","continue","export","local","readonly","declare","unset","set","source","alias","unalias"]),
      extraRules: [
        { pattern: String.raw`\$\{[^}]*\}`, cls: "var" },
        { pattern: String.raw`\$\w+`, cls: "var" },
        { pattern: String.raw`\b(?:true|false)\b`, cls: "bool" },
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`#!?\s*/[a-z/]+`, cls: "shebang" }, // shebang
      ],
    },

    html: {
      blockComment: ["<!--", "-->"],
      strings: [{ open: '"', close: '"', escape: null }, { open: "'", close: "'", escape: null }],
      keywords: new Set([]),
      extraRules: [
        { pattern: String.raw`</?[A-Za-z][A-Za-z0-9-]*`, cls: "tag" },
        { pattern: String.raw`/?>`, cls: "tag" },
        { pattern: String.raw`\s([A-Za-z-]+)(=)`, cls: "attr" },
        { pattern: String.raw`"[^"]*"`, cls: "string" },  // attribute value
        { pattern: String.raw`\bdoctype\b`, cls: "keyword", ci: true },
      ],
    },

    css: {
      blockComment: ["/*", "*/"],
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }],
      keywords: new Set(["important","default","inherit","initial","unset","revert","none","auto","hidden","visible","block","inline","inline-block","flex","grid","table","absolute","relative","fixed","static","sticky"]),
      extraRules: [
        { pattern: String.raw`-?[A-Za-z-]+(?=\s*:)`, cls: "selector" },
        { pattern: String.raw`#[\w-]+`, cls: "idsel" },
        { pattern: String.raw`\.[\w-]+`, cls: "classsel" },
        { pattern: RE_NUMBER, cls: "num" },
      ],
    },

    json: {
      blockComment: null,
      strings: [{ open: '"', close: '"', escape: "\\" }],
      keywords: new Set(["true","false","null"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`"(?:\\.|[^"\\])*"(?=\s*:)`, cls: "key" }, // JSON key
      ],
    },

    markdown: {
      blockComment: null,
      strings: [],
      keywords: new Set([]),
      extraRules: [
        { pattern: String.raw`^(#{1,6})\s`, cls: "heading" },
        { pattern: String.raw`\*\*[^*]+\*\*`, cls: "bold" },
        { pattern: String.raw`__[^_]+__`, cls: "bold" },
        { pattern: String.raw`\*[^*]+\*`, cls: "italic" },
        { pattern: String.raw`_[^_]+_`, cls: "italic" },
        { pattern: String.raw`\`[^\`]+\``, cls: "code" },
        { pattern: String.raw`\`\`\`[\s\S]*?\`\`\``, cls: "codeblock" },
        { pattern: String.raw`!\[[^\]]*\]\([^)]+\)`, cls: "imglink" },
        { pattern: String.raw`\[([^\]]+)\]\(([^)]+)\)`, cls: "link" },
        { pattern: String.raw`^\s*[-*+]\s+`, cls: "bullet" },
        { pattern: String.raw`^\s*\d+\.\s+`, cls: "bullet" },
        { pattern: String.raw`^\s*>\s.*$`, cls: "quote" },
        { pattern: String.raw`^---+$`, cls: "hr" },
      ],
    },

    yaml: {
      lineComment: "#",
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: null }],
      keywords: new Set(["true","false","yes","no","on","off","null","~"]),
      extraRules: [
        { pattern: String.raw`^[A-Za-z_][A-Za-z0-9_-]*(?=:)` , cls: "key" },
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`^\s*-\s+`, cls: "bullet" },
      ],
    },

    toml: {
      lineComment: "#",
      strings: [{ open: '"', close: '"', escape: "\\" }, { open: "'", close: "'", escape: "\\" }, { open: '"""', close: '"""', escape: "\\" }],
      keywords: new Set(["true","false"]),
      extraRules: [
        { pattern: String.raw`^\s*\[[A-Za-z0-9_.\-]+\]\s*$`, cls: "section" },
        { pattern: String.raw`^[A-Za-z_][A-Za-z0-9_-]*(?==)`, cls: "key" },
        { pattern: RE_NUMBER, cls: "num" },
      ],
    },

    sql: {
      lineComment: "--",
      blockComment: ["/*", "*/"],
      strings: [{ open: "'", close: "'", escape: "''" }, { open: '"', close: '"', escape: '""' }],
      keywords: new Set(["SELECT","FROM","WHERE","INSERT","INTO","VALUES","UPDATE","SET","DELETE","CREATE","TABLE","INDEX","VIEW","DROP","ALTER","ADD","COLUMN","PRIMARY","KEY","FOREIGN","REFERENCES","JOIN","INNER","LEFT","RIGHT","OUTER","FULL","ON","AS","AND","OR","NOT","NULL","IS","IN","BETWEEN","LIKE","EXISTS","HAVING","GROUP","BY","ORDER","LIMIT","OFFSET","UNION","ALL","DISTINCT","CASE","WHEN","THEN","ELSE","END","WITH","RECURSIVE","BEGIN","COMMIT","ROLLBACK","TRANSACTION","SELECT","INT","INTEGER","VARCHAR","CHAR","TEXT","DATE","TIMESTAMP","BOOLEAN","FLOAT","DOUBLE"]),
      extraRules: [
        { pattern: RE_NUMBER, cls: "num" },
        { pattern: String.raw`\b(?:true|false|null)\b`, cls: "bool", ci: true },
      ],
    },

    plain: { lineComment: null, strings: [], keywords: new Set(), extraRules: [] },
  };

  // ===== 高亮一行 =====
  // 把 text 切成 [{text, cls}] 数组 (cls 是 CSS class). cls null 表示普通文本.
  function highlightLine(text, lang) {
    if (!text || lang === "plain" || !LANGS[lang]) return [{ text, cls: null }];
    const lexer = LANGS[lang];
    const result = [];
    let i = 0;
    const len = text.length;

    while (i < len) {
      // 1. 行注释 (优先级最高)
      if (lexer.lineComment && text.startsWith(lexer.lineComment, i)) {
        result.push({ text: text.slice(i), cls: "comment" });
        return result;
      }
      // 2. 块注释起始 (单行情况 — 真正多行要跨行跟踪, 这里只做单行)
      let matched = false;
      if (lexer.blockComment) {
        const [bcOpen, bcClose] = lexer.blockComment;
        if (text.startsWith(bcOpen, i)) {
          const end = text.indexOf(bcClose, i + bcOpen.length);
          const endIdx = end < 0 ? len : end + bcClose.length;
          result.push({ text: text.slice(i, endIdx), cls: "comment" });
          i = endIdx;
          continue;
        }
      }
      // 3. 字符串
      if (lexer.strings) {
        for (const s of lexer.strings) {
          if (text.startsWith(s.open, i)) {
            let j = i + s.open.length;
            const endStr = s.close;
            while (j < len) {
              if (s.escape && text[j] === s.escape && j + 1 < len) { j += 2; continue; }
              if (text.startsWith(endStr, j)) { j += endStr.length; break; }
              j++;
            }
            result.push({ text: text.slice(i, j), cls: s.raw ? "rawstring" : (s.template ? "template" : "string") });
            i = j;
            matched = true;
            break;
          }
        }
        if (matched) continue;
      }
      // 4. extraRules (按声明顺序尝试, 先匹配先得)
      let ruleMatched = false;
      if (lexer.extraRules) {
        for (const rule of lexer.extraRules) {
          const re = new RegExp("^(?:" + rule.pattern + ")");
          const m = text.slice(i).match(re);
          if (m) {
            // 检测 within 限定 (e.g. fmt within fstring): 我们已经 escape 字符串, 不会到这里
            result.push({ text: m[0], cls: rule.cls });
            i += m[0].length;
            ruleMatched = true;
            break;
          }
        }
        if (ruleMatched) continue;
      }
      // 5. 数字
      const numRe = new RegExp("^(?:" + RE_NUMBER + ")");
      const nm = text.slice(i).match(numRe);
      if (nm) {
        result.push({ text: nm[0], cls: "num" });
        i += nm[0].length;
        continue;
      }
      // 6. 标识符 / 关键字 / 类型
      const idRe = /^[A-Za-z_][A-Za-z0-9_]*/;
      const im = text.slice(i).match(idRe);
      if (im) {
        const word = im[0];
        let cls = null;
        if (lexer.keywords && lexer.keywords.has(word)) cls = "keyword";
        else if (lexer.types && lexer.types.has(word)) cls = "type";
        if (cls) result.push({ text: word, cls });
        else result.push({ text: word, cls: null });
        i += word.length;
        continue;
      }
      // 7. 单字符 (操作符 / 标点)
      result.push({ text: text[i], cls: "op" });
      i++;
    }
    return result;
  }

  // ===== 暴露 API =====
  global.Syntax = {
    detectLanguage,
    languageLabel,
    highlightLine,
    LANGUAGES: Object.keys(LANGS),
  };
})(typeof window !== "undefined" ? window : globalThis);
