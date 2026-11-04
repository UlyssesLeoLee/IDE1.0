// Temporary esbuild entry: graphology + dagre-graphlib -> globals `Graphology` / `Dagre`
//
// `dagre-graphlib` is NOT published on the npm registry (404). The published
// equivalent is the `dagre` package (MIT), whose `module.exports.graphlib` IS
// the npm `graphlib` package itself (verified: require('dagre').graphlib === require('graphlib')).
// `dagre` is CommonJS, so we use a default import to get module.exports verbatim.
//
// `export * from 'graphology'` re-exports the named bindings (Graph, DirectedGraph,
// MultiGraph, ...) but NOT the default export — `Graphology.Graph` is what we need.
export * from 'graphology';
import dagre from 'dagre';
export { dagre };
