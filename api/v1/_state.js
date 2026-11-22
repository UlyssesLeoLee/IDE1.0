// Shared Serverless Gateway Facade (exports durable Postgres store & merge primitives)

import {
  checkSyntaxIntegrity,
  diffLines,
  formatUnifiedDiff,
  nemaSemanticMerge,
  parseNemaStrict,
  splitLines,
  threeWayMerge,
  validateNemaSyntax,
} from "./_merge.js";
import { getStore, PgDurableStore, sha256Hex, StoreError } from "./_store.js";

export {
  checkSyntaxIntegrity,
  diffLines,
  formatUnifiedDiff,
  getStore,
  nemaSemanticMerge,
  parseNemaStrict,
  PgDurableStore,
  sha256Hex,
  splitLines,
  StoreError,
  threeWayMerge,
  validateNemaSyntax,
};
