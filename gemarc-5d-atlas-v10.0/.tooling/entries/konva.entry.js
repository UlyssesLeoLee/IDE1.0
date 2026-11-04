// Temporary esbuild entry: Konva.js -> global `Konva`
//
// konva/lib/index.js only has a *default* export (`export default Konva`).
// esbuild's `--global-name` always assigns the MODULE NAMESPACE OBJECT to the
// global, so `--global-name=Konva` would yield global `Konva === {Konva: <ns>}`
// and `Konva.Stage` would be undefined.
// Fix: export the namespace under the name `Konva` and unwrap in the esbuild
// footer (`var Konva = Konva.Konva;`), reproducing Konva's classic UMD global API
// where `Konva.Stage` / `Konva.Layer` / `Konva.Rect` are direct members.
export * from 'konva';
export { default as Konva } from 'konva';
