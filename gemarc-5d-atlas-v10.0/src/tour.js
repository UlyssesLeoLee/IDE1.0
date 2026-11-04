/**
 * src/tour.js — 2D 流程动画 / 导览播放器（降低认知负荷）
 *
 * 契约（v10.0 information-placement + 用户要求）：
 *   - 导览可跨维度（step.view），用来演示维度依赖链 1D→2D→3D→4D→5D。
 *   - 每步只聚焦少量元素（≤6 节点 / ≤8 边），并给一句"该看什么"的提示。
 *   - 尊重 prefers-reduced-motion：开启时不做镜头缓动，只做高亮切换。
 *   - 随时可暂停 / 退出；退出后各维度仍可自由使用。
 */
(function (root) {
  "use strict";

  var S = {
    tours: [],
    tour: null,
    index: -1,
    playing: false,
    timer: null,
    raf: null,
    onView: null,
    onFocus: null,
    onCamera: null,
    onHighlight: null,
    onState: null
  };

  function reducedMotion() {
    return !!(root.matchMedia && root.matchMedia("(prefers-reduced-motion: reduce)").matches);
  }

  function load(tours) {
    S.tours = (tours && tours.tours) || [];
  }

  function emit() {
    // state() 是模块级函数，不在 S 上 —— 写成 S.state() 会抛 TypeError
    if (S.onState) S.onState(state());
  }

  function clearTimers() {
    if (S.timer) { clearTimeout(S.timer); S.timer = null; }
    if (S.raf) { cancelAnimationFrame(S.raf); S.raf = null; }
  }

  /** 应用一步的高亮与镜头 */
  function applyStep(step) {
    if (!step) return;
    if (S.onView && step.view) S.onView(step.view);
    if (S.onHighlight) S.onHighlight(step.focus || {});
    if (S.onCamera && step.camera) {
      S.onCamera(step.camera, reducedMotion());
    }
  }

  function goto(i, auto) {
    if (!S.tour) return;
    var steps = S.tour.steps || [];
    if (i < 0) i = 0;
    if (i >= steps.length) { stop(); return; }
    S.index = i;
    var step = steps[i];
    applyStep(step);
    emit();
    if (auto !== false && S.playing) {
      clearTimers();
      S.timer = setTimeout(function () { goto(i + 1, true); }, step.durationMs || 7000);
    }
  }

  function start(tourId) {
    var t = S.tours.filter(function (x) { return x.id === tourId; })[0];
    if (!t) return false;
    clearTimers();
    S.tour = t;
    S.playing = true;
    S.index = -1;
    goto(0, true);
    return true;
  }

  function next() { goto(S.index + 1, true); }
  function prev() { goto(S.index - 1, true); }

  function pause() {
    S.playing = false;
    clearTimers();
    emit();
  }

  function resume() {
    if (!S.tour) return;
    S.playing = true;
    var step = (S.tour.steps || [])[S.index];
    if (step) {
      clearTimers();
      S.timer = setTimeout(function () { goto(S.index + 1, true); }, step.durationMs || 7000);
    }
    emit();
  }

  function toggle() { if (S.playing) pause(); else resume(); }

  function stop() {
    clearTimers();
    S.tour = null;
    S.index = -1;
    S.playing = false;
    if (S.onHighlight) S.onHighlight({});
    emit();
  }

  function state() {
    var steps = (S.tour && S.tour.steps) || [];
    var step = steps[S.index] || null;
    return {
      active: !!S.tour,
      tourId: S.tour ? S.tour.id : null,
      tourTitle: S.tour ? S.tour.title : null,
      index: S.index,
      total: steps.length,
      playing: S.playing,
      step: step
    };
  }

  /**
   * 相机缓动：把 2D 画布的 tx/ty/scale 平滑移到目标。
   * 落在 app.js 的 drawCanvas 钩子上 —— tour.js 只算插值，不碰 DOM。
   */
  function easeOut(t) { return 1 - Math.pow(1 - t, 3); }

  function animateCamera(from, to, durationMs, onFrame) {
    if (reducedMotion()) { onFrame(to); return; }
    var t0 = null;
    var dur = Math.max(120, durationMs || 520);
    function tick(ts) {
      if (t0 === null) t0 = ts;
      var p = Math.min(1, (ts - t0) / dur);
      var e = easeOut(p);
      onFrame({
        x: from.x + (to.x - from.x) * e,
        y: from.y + (to.y - from.y) * e,
        scale: from.scale + (to.scale - from.scale) * e
      });
      if (p < 1) S.raf = requestAnimationFrame(tick);
    }
    S.raf = requestAnimationFrame(tick);
  }

  var API = {
    load: load,
    start: start,
    next: next,
    prev: prev,
    pause: pause,
    resume: resume,
    toggle: toggle,
    stop: stop,
    state: state,
    animateCamera: animateCamera,
    reducedMotion: reducedMotion,
    setHandlers: function (h) {
      S.onView = h.onView;
      S.onFocus = h.onFocus;
      S.onCamera = h.onCamera;
      S.onHighlight = h.onHighlight;
      S.onState = h.onState;
    },
    list: function () { return S.tours; }
  };

  if (typeof module !== "undefined" && module.exports) module.exports = API;
  root.GemArcTour = API;
})(typeof globalThis !== "undefined" ? globalThis : this);
