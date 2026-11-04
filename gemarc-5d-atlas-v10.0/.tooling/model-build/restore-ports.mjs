import fs from "node:fs";
const ROOT = "E:/IDE1.0/gemarc-5d-atlas-v10.0";
const p = ROOT + "/models/architecture.json";
const a = JSON.parse(fs.readFileSync(p, "utf8"));

// 恢复**人工登记的端口**（边的 sourcePortId/targetPortId 依赖这些 ID）。
// 这些是设计时挑的"代表性接口"，ID 不能变；sync-ports.mjs 会按 name 复用它们并补齐其余真实接口。
const HTTP = [
  ["P-HTTP-DOC-OPEN", "POST /api/doc/open"], ["P-HTTP-DOC-TABS", "GET /api/doc/tabs"],
  ["P-HTTP-DOC-ACTIVE", "GET /api/doc/active"], ["P-HTTP-DOC-ACTIVATE", "POST /api/doc/activate"],
  ["P-HTTP-DOC-CLOSE", "POST /api/doc/close"], ["P-HTTP-DOC-NEWTAB", "POST /api/doc/new_tab"],
  ["P-HTTP-DOC-INSERT", "POST /api/doc/insert"], ["P-HTTP-DOC-TYPE", "POST /api/doc/type"],
  ["P-HTTP-DOC-NEWLINE", "POST /api/doc/newline"], ["P-HTTP-DOC-BACKSPACE", "POST /api/doc/backspace"],
  ["P-HTTP-DOC-DELETE", "POST /api/doc/delete"], ["P-HTTP-DOC-DELETERANGE", "POST /api/doc/delete_range"],
  ["P-HTTP-DOC-CARET", "POST /api/doc/caret"], ["P-HTTP-DOC-SELECTALL", "POST /api/doc/select_all"],
  ["P-HTTP-DOC-UNDO", "POST /api/doc/undo"], ["P-HTTP-DOC-REDO", "POST /api/doc/redo"],
  ["P-HTTP-DOC-FIND", "POST /api/doc/find"], ["P-HTTP-DOC-FINDALL", "POST /api/doc/find_all"],
  ["P-HTTP-DOC-REPLACEALL", "POST /api/doc/replace_all"], ["P-HTTP-DOC-LINEENDING", "POST /api/doc/line_ending"],
  ["P-HTTP-DOC-ENCODING", "POST /api/doc/encoding"], ["P-HTTP-DOC-EXPORT", "GET /api/doc/export"],
  ["P-HTTP-DOC-MARKSAVED", "POST /api/doc/mark_saved"],
  ["P-HTTP-TERM-CREATE", "POST /api/terminal_create"], ["P-HTTP-TERM-INPUT", "POST /api/terminal_input"],
  ["P-HTTP-TERM-CLOSE", "POST /api/terminal_close"], ["P-HTTP-TERM-OUTPUT", "GET /api/terminal_output"],
  ["P-HTTP-TERM-LIST", "GET /api/terminal_list"],
  ["P-HTTP-SCM-STATUS", "POST /api/scm_status"], ["P-HTTP-SCM-STAGE", "POST /api/scm_stage"],
  ["P-HTTP-SCM-UNSTAGE", "POST /api/scm_unstage"], ["P-HTTP-SCM-DIFF", "POST /api/scm_diff"],
  ["P-HTTP-SCM-COMMIT", "POST /api/scm_commit"], ["P-HTTP-SCM-LOG", "POST /api/scm_log"],
  ["P-HTTP-SEARCH", "POST /api/search"], ["P-HTTP-OUTLINE", "POST /api/outline"],
  ["P-HTTP-FRAME", "GET /api/frame"], ["P-HTTP-EVENT", "POST /api/event"],
  ["P-HTTP-KEY", "POST /api/key"], ["P-HTTP-MOUSE", "POST /api/mouse"],
  ["P-HTTP-OPENPROJECT", "POST /api/open_project"], ["P-HTTP-LISTDIR", "POST /api/list_dir"],
  ["P-HTTP-READFILE", "POST /api/read_file"], ["P-HTTP-WRITEFILE", "POST /api/write_file"],
  ["P-HTTP-PLUGINLOAD", "POST /api/plugin_load"]
];
const TAURI = [
  ["P-TAURI-DOC-ACTIVE", "doc_active"], ["P-TAURI-DOC-TABS", "doc_tabs"], ["P-TAURI-DOC-OPEN", "doc_open"],
  ["P-TAURI-DOC-NEWTAB", "doc_new_tab"], ["P-TAURI-DOC-ACTIVATE", "doc_activate"], ["P-TAURI-DOC-CLOSE", "doc_close"],
  ["P-TAURI-DOC-INSERT", "doc_insert"], ["P-TAURI-DOC-UNDO", "doc_undo"], ["P-TAURI-DOC-REDO", "doc_redo"],
  ["P-TAURI-DOC-FIND", "doc_find"], ["P-TAURI-DOC-REPLACEALL", "doc_replace_all"], ["P-TAURI-DOC-EXPORT", "doc_export"],
  ["P-TAURI-TERM-CREATE", "terminal_create"], ["P-TAURI-TERM-INPUT", "terminal_input"],
  ["P-TAURI-TERM-CLOSE", "terminal_close"], ["P-TAURI-TERM-LIST", "terminal_list"],
  ["P-TAURI-SCM-STATUS", "scm_status"], ["P-TAURI-SCM-STAGE", "scm_stage"], ["P-TAURI-SCM-COMMIT", "scm_commit"],
  ["P-TAURI-SCM-DIFF", "scm_diff"], ["P-TAURI-SCM-LOG", "scm_log"],
  ["P-TAURI-SEARCH", "search_cmd"], ["P-TAURI-OUTLINE", "outline"], ["P-TAURI-PLUGINLOAD", "plugin_load"],
  ["P-TAURI-LUMINANCE", "desktop_luminance"], ["P-TAURI-THEMEVARS", "clear_theme_vars"]
];
const RPC = [
  ["P-RPC-KERNELINFO", "kernel_info"], ["P-RPC-OPENBUFFER", "open_buffer"], ["P-RPC-CLOSEBUFFER", "close_buffer"],
  ["P-RPC-LISTBUFFERS", "list_buffers"], ["P-RPC-SETACTIVEBUFFER", "set_active_buffer"],
  ["P-RPC-ACTIVEBUFFERID", "active_buffer_id"], ["P-RPC-GETBUFFER", "get_buffer"],
  ["P-RPC-SETBUFFERLINES", "set_buffer_lines"], ["P-RPC-INSERTCURSOR", "insert_at_cursor"],
  ["P-RPC-KEYEVENT", "key_event"], ["P-RPC-SETCURSOR", "set_cursor"], ["P-RPC-EXECUTECOMMAND", "execute_command"],
  ["P-RPC-SETCMDTEXT", "set_cmd_text"], ["P-RPC-GETCMDTEXT", "get_cmd_text"],
  ["P-RPC-COMPLETEATCURSOR", "complete_at_cursor"], ["P-RPC-EDITVIAPROVIDER", "edit_via_provider"],
  ["P-RPC-CHAT", "chat"], ["P-RPC-REGISTERPROVIDER", "register_provider"],
  ["P-RPC-UNREGISTERPROVIDER", "unregister_provider"], ["P-RPC-LISTPROVIDERS", "list_providers"]
];
const mk = (id, name, nodeId, type) => ({ id, name, direction: "in", type, nodeId });
const restored = []
  .concat(HTTP.map((x) => mk(x[0], x[1], "N-WB-SERVER", "HTTP")))
  .concat(TAURI.map((x) => mk(x[0], x[1], "N-DT-TAURI", "Tauri command")))
  .concat(RPC.map((x) => mk(x[0], x[1], "N-PT-RPC", "JSON-RPC 2.0 method")));
const have = new Set(a.ports.map((x) => x.id));
for (const r of restored) if (!have.has(r.id)) a.ports.push(r);
fs.writeFileSync(p, JSON.stringify(a, null, 2), "utf8");
console.log("恢复人工端口:", restored.length, "→ 总端口", a.ports.length);
