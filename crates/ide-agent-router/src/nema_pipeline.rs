//! `nema_pipeline` — IDE1.0 多智能体协同开发平台流水线核心引擎。
//!
//! 实现架构规范 [BD-002] 与 [DD-002] 定义的四阶段流水线：
//! - ① **路由拆解 (`Router`)**：任务分类、文件分配、协同/独立编组、分支规划与依赖 DAG
//! - ② **并行协作 (`.nema`)**：多分支租约、Fencing Token、原子 CAS 与 GuidanceZone 契约
//! - ③ **统筹转换 (`Aggregator`)**：共享词典归一重排、三方语义合并、`.nema → Rust` 降维与 AST 漂移守卫
//! - ④ **Git 管理与验证 (`Git & Verify`)**：文件级分支拓扑、四道门禁流水线、证书签发与闭环自愈回流

use std::collections::{BTreeMap, BTreeSet};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use nema_codec::{
    DomainMode, EdgeDirection, NemaCodec, NemaDocument, NemaError,
    NemaRecord, StandardNemaCodec, TextSpan,
};
use crate::agent_profile::{AgentSlotId, FeedbackDecision, FeedbackLoopGuard};

// ============================================================================
// 阶段 ①：路由拆解 (Router Agent)
// ============================================================================

/// 任务输入类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskInputKind {
    /// 功能开发
    FeatureDevelopment,
    /// 缺陷修复
    DefectRepair,
    /// 模块优化
    ModuleOptimization,
    /// 重构升级
    RefactoringUpgrade,
}

impl TaskInputKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::FeatureDevelopment => "功能开发",
            Self::DefectRepair => "缺陷修复",
            Self::ModuleOptimization => "模块优化",
            Self::RefactoringUpgrade => "重构升级",
        }
    }
}

/// 协同编组类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CollaborationGroupKind {
    /// 协同编辑组 A：多 Agent 在不同分支 (feat/A1, feat/A2) 共同协作同一 .nema 文件
    CoEditingSameFile {
        nema_file: String,
        target_rust_file: String,
        assignments: Vec<CoEditBranchAssignment>,
    },
    /// 独立编辑组 B：各 Agent 在各自独立分支 (feat/B1, feat/B2) 独占编辑独立 .nema 文件
    IndependentFiles {
        assignments: Vec<IndependentBranchAssignment>,
    },
}

/// 同文件协同分支分配契约
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoEditBranchAssignment {
    pub slot_id: AgentSlotId,
    pub agent_id: String,
    pub branch_name: String,
    pub allowed_domains: Vec<String>,
    pub record_key_prefix: Option<String>,
    pub guidance_zone: String,
    pub fencing_token: u64,
}

/// 独立文件独占分支分配契约
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndependentBranchAssignment {
    pub slot_id: AgentSlotId,
    pub agent_id: String,
    pub branch_name: String,
    pub nema_file: String,
    pub target_rust_file: String,
    pub guidance_zone: String,
    pub fencing_token: u64,
}

/// 任务依赖 DAG 边
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from_branch: String,
    pub to_branch: String,
    pub dependency_reason: String,
}

/// 路由智能体生成的任务拆解与分支规划总纲
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskDecompositionPlan {
    pub pipeline_run_id: String,
    pub epoch: u64,
    pub task_kind: TaskInputKind,
    pub summary: String,
    pub groups: Vec<CollaborationGroupKind>,
    pub dependency_dag: Vec<DependencyEdge>,
    pub created_at: DateTime<Utc>,
}

/// 路由智能体引擎
#[derive(Debug, Default)]
pub struct RouterEngine;

impl RouterEngine {
    /// 根据任务输入、关联文件与描述生成确定性的任务拆解计划
    pub fn plan(
        &self,
        pipeline_run_id: &str,
        epoch: u64,
        task_kind: TaskInputKind,
        summary: &str,
        target_files: &[String],
    ) -> TaskDecompositionPlan {
        let mut groups = Vec::new();
        let mut dependency_dag = Vec::new();

        // 判定规则：若目标文件中包含主规格文件（如 File A.nema 或相同前缀），划分协同组 A；其余为独立组 B
        let (shared_targets, independent_targets): (Vec<_>, Vec<_>) = target_files
            .iter()
            .cloned()
            .partition(|f| f.contains("file_a") || f.contains("File_A") || f.contains("core"));

        let shared_file = shared_targets.first().cloned().unwrap_or_else(|| {
            if target_files.is_empty() {
                "specs/File_A.nema".to_string()
            } else {
                target_files[0].clone()
            }
        });

        let target_rust_a = shared_file
            .replace("specs/", "src/")
            .replace(".nema", ".rs");

        // 协同编辑组 A (Agent A1 & Agent A2 协同编辑同一文件)
        let co_edit_group = CollaborationGroupKind::CoEditingSameFile {
            nema_file: shared_file.clone(),
            target_rust_file: target_rust_a,
            assignments: vec![
                CoEditBranchAssignment {
                    slot_id: AgentSlotId::AgentA1,
                    agent_id: "Agent-A1".to_string(),
                    branch_name: "feat/A1".to_string(),
                    allowed_domains: vec!["1".to_string(), "2".to_string()],
                    record_key_prefix: None,
                    guidance_zone: "Table structures and 5W1H specification contracts".to_string(),
                    fencing_token: epoch * 100 + 1,
                },
                CoEditBranchAssignment {
                    slot_id: AgentSlotId::AgentA2,
                    agent_id: "Agent-A2".to_string(),
                    branch_name: "feat/A2".to_string(),
                    allowed_domains: vec!["3".to_string()],
                    record_key_prefix: None,
                    guidance_zone: "Graph module topology and state transitions".to_string(),
                    fencing_token: epoch * 100 + 2,
                },
            ],
        };
        groups.push(co_edit_group);

        // 独立编辑组 B (Agent B1 & Agent B2 独占独立文件)
        let mut indep_assignments = Vec::new();
        let b1_file = independent_targets.first().cloned().unwrap_or_else(|| "specs/File_B.nema".to_string());
        let b1_rust = b1_file.replace("specs/", "src/").replace(".nema", ".rs");
        indep_assignments.push(IndependentBranchAssignment {
            slot_id: AgentSlotId::AgentB1,
            agent_id: "Agent-B1".to_string(),
            branch_name: "feat/B1".to_string(),
            nema_file: b1_file,
            target_rust_file: b1_rust,
            guidance_zone: "Auxiliary protocol implementation".to_string(),
            fencing_token: epoch * 100 + 3,
        });

        let b2_file = independent_targets.get(1).cloned().unwrap_or_else(|| "specs/File_C.nema".to_string());
        let b2_rust = b2_file.replace("specs/", "src/").replace(".nema", ".rs");
        indep_assignments.push(IndependentBranchAssignment {
            slot_id: AgentSlotId::AgentB2,
            agent_id: "Agent-B2".to_string(),
            branch_name: "feat/B2".to_string(),
            nema_file: b2_file,
            target_rust_file: b2_rust,
            guidance_zone: "Telemetry and event dispatcher".to_string(),
            fencing_token: epoch * 100 + 4,
        });

        groups.push(CollaborationGroupKind::IndependentFiles {
            assignments: indep_assignments,
        });

        // 依赖 DAG：feat/A1 提供数据结构契约 -> feat/A2 拓扑依赖 -> feat/B1 跨模块调用
        dependency_dag.push(DependencyEdge {
            from_branch: "feat/A1".to_string(),
            to_branch: "feat/A2".to_string(),
            dependency_reason: "Topology references type contracts defined in domains 1 and 2".to_string(),
        });
        dependency_dag.push(DependencyEdge {
            from_branch: "feat/A1".to_string(),
            to_branch: "feat/B1".to_string(),
            dependency_reason: "File B consumes types exported from File A".to_string(),
        });

        TaskDecompositionPlan {
            pipeline_run_id: pipeline_run_id.to_string(),
            epoch,
            task_kind,
            summary: summary.to_string(),
            groups,
            dependency_dag,
            created_at: Utc::now(),
        }
    }
}

// ============================================================================
// 阶段 ③：统筹智能体 (Aggregator Agent) 语义合并与代码降维
// ============================================================================

/// 冲突消解策略
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictResolutionStrategy {
    PreferBranchLhs,
    PreferBranchRhs,
    UnionMerge,
    GuidanceZoneRule,
}

/// 语义级冲突记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NemaSemanticConflict {
    pub file_path: String,
    pub domain_id: String,
    pub record_key: String,
    pub column_index: usize,
    pub branch_lhs: (String, String),
    pub branch_rhs: (String, String),
    pub resolution: ConflictResolutionStrategy,
}

/// 多分支合并输出
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NemaMergeOutcome {
    pub file_path: String,
    pub rendered_nema_source: String,
    pub remapped_dictionary: Vec<String>,
    pub conflicts: Vec<NemaSemanticConflict>,
}

/// 统筹智能体三方语义合并器
#[derive(Debug, Default)]
pub struct NemaSemanticMerger;

impl NemaSemanticMerger {
    /// 合并两个并发分支的 NEMA 文档（Step 1: 词典归一，Step 2: 域级路由，Step 3: 记录/边合并）
    pub fn merge(
        &self,
        file_path: &str,
        base_src: &str,
        branch_a_name: &str,
        branch_a_src: &str,
        branch_b_name: &str,
        branch_b_src: &str,
    ) -> Result<NemaMergeOutcome, NemaError> {
        let codec = StandardNemaCodec;
        let base_doc = codec.parse(base_src).unwrap_or_else(|_| NemaDocument {
            span: TextSpan::default(),
            dictionary: Vec::new(),
            dictionary_spans: Vec::new(),
            domains: Vec::new(),
        });
        let doc_a = codec.parse(branch_a_src)?;
        let doc_b = codec.parse(branch_b_src)?;

        // Step 1: 共享词典归一化重排与去重
        let mut unified_dict_set = BTreeSet::new();
        for d in &base_doc.dictionary {
            unified_dict_set.insert(d.clone());
        }
        for d in &doc_a.dictionary {
            unified_dict_set.insert(d.clone());
        }
        for d in &doc_b.dictionary {
            unified_dict_set.insert(d.clone());
        }
        let merged_dict: Vec<String> = unified_dict_set.into_iter().collect();

        // Step 2 & 3: 域级与记录级三方合并
        let mut merged_domains = Vec::new();
        let mut all_domain_ids = BTreeSet::new();
        for dom in &doc_a.domains {
            all_domain_ids.insert(dom.id.clone());
        }
        for dom in &doc_b.domains {
            all_domain_ids.insert(dom.id.clone());
        }
        for dom in &base_doc.domains {
            all_domain_ids.insert(dom.id.clone());
        }

        let mut conflicts = Vec::new();

        for dom_id in all_domain_ids {
            let in_a = doc_a.domains.iter().find(|d| d.id == dom_id);
            let in_b = doc_b.domains.iter().find(|d| d.id == dom_id);

            match (in_a, in_b) {
                // 仅分支 A 拥有该域 -> 直接接纳 A
                (Some(da), None) => merged_domains.push(da.clone()),
                // 仅分支 B 拥有该域 -> 直接接纳 B
                (None, Some(db)) => merged_domains.push(db.clone()),
                // 两分支并发修改同一域 -> 记录/边级三方合并
                (Some(da), Some(db)) => {
                    let mut combined_domain = da.clone();
                    combined_domain.revision = combined_domain.revision.max(db.revision) + 1;

                    let mut record_map: BTreeMap<String, NemaRecord> = BTreeMap::new();
                    // 填入 A 的记录
                    for r in &da.records {
                        let key = r
                            .cells
                            .first()
                            .and_then(|c| c.first())
                            .map(|chunk| chunk.raw.clone())
                            .unwrap_or_default();
                        record_map.insert(key, r.clone());
                    }
                    // 合并 B 的记录
                    for r in &db.records {
                        let key = r
                            .cells
                            .first()
                            .and_then(|c| c.first())
                            .map(|chunk| chunk.raw.clone())
                            .unwrap_or_default();
                        if let Some(existing_rec) = record_map.get_mut(&key) {
                            // 同一键：合并不同列；相同列不一致时生成冲突消解
                            for (c_idx, col_b) in r.cells.iter().enumerate() {
                                if c_idx >= existing_rec.cells.len() {
                                    existing_rec.cells.push(col_b.clone());
                                } else {
                                    let val_a = existing_rec.cells[c_idx]
                                        .iter()
                                        .map(|c| c.raw.as_str())
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    let val_b = col_b
                                        .iter()
                                        .map(|c| c.raw.as_str())
                                        .collect::<Vec<_>>()
                                        .join(" ");
                                    if val_a != val_b {
                                        // 触发细粒度冲突裁决：GuidanceZone 规则默认采纳 A 或保留联合
                                        conflicts.push(NemaSemanticConflict {
                                            file_path: file_path.to_string(),
                                            domain_id: dom_id.clone(),
                                            record_key: key.clone(),
                                            column_index: c_idx,
                                            branch_lhs: (branch_a_name.to_string(), val_a),
                                            branch_rhs: (branch_b_name.to_string(), val_b),
                                            resolution: ConflictResolutionStrategy::GuidanceZoneRule,
                                        });
                                    }
                                }
                            }
                        } else {
                            record_map.insert(key, r.clone());
                        }
                    }

                    combined_domain.records = record_map.into_values().collect();
                    merged_domains.push(combined_domain);
                }
                (None, None) => {}
            }
        }

        // 渲染成规范 NEMA 格式
        let mut rendered = String::new();
        if !merged_dict.is_empty() {
            rendered.push_str(&format!("\t${}\n", merged_dict.len()));
            for item in &merged_dict {
                rendered.push_str(item);
                rendered.push('\n');
            }
        }

        for dom in &merged_domains {
            rendered.push_str(&format!("\t{}{}\n", dom.mode.prefix_char(), dom.id));
            if dom.explicit_header && !dom.columns.is_empty() {
                rendered.push_str("\\ ");
                rendered.push_str(&dom.columns.join(" "));
                rendered.push('\n');
            }
            for rec in &dom.records {
                if let Some(edge) = &rec.graph_edge {
                    let op = match edge.direction {
                        EdgeDirection::Directed => edge.raw_operator,
                        EdgeDirection::Undirected => '=',
                    };
                    rendered.push_str(&format!("{}{}{}{}{} Description\n", edge.from, op, edge.id, op, edge.to));
                } else if let Some(node_id) = &rec.graph_node_id {
                    let desc = rec.raw_groups().into_iter().flatten().skip(1).collect::<Vec<_>>().join(" ");
                    if desc.is_empty() {
                        rendered.push_str(&format!("{node_id} Node\n"));
                    } else {
                        rendered.push_str(&format!("{node_id} {desc}\n"));
                    }
                } else {
                    let mut line = String::new();
                    for (i, col) in rec.cells.iter().enumerate() {
                        if i > 0 {
                            line.push(' ');
                        }
                        let col_str = col.iter().map(|c| c.raw.as_str()).collect::<Vec<_>>().join(" ");
                        line.push_str(&col_str);
                    }
                    rendered.push_str(&line);
                    rendered.push('\n');
                }
            }
        }
        rendered.push_str("\n\n\n");

        Ok(NemaMergeOutcome {
            file_path: file_path.to_string(),
            rendered_nema_source: rendered,
            remapped_dictionary: merged_dict,
            conflicts,
        })
    }
}

// ============================================================================
// 步骤 ③：.nema → Rust 结构化降维 (Lowering Engine) & SourceMap
// ============================================================================

/// 双向 SourceMap 映射条目
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NemaRustSourceMapEntry {
    pub nema_file: String,
    pub domain_id: String,
    pub domain_mode: DomainMode,
    pub record_key: String,
    pub nema_span: TextSpan,
    pub source_branch: String,
    pub rust_file: String,
    pub rust_symbol_path: String,
    pub rust_line_range: (usize, usize),
    pub generated_rust_ast_hash: String,
}

/// 双向 SourceMap 映射表
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NemaRustSourceMap {
    pub entries: Vec<NemaRustSourceMapEntry>,
}

impl NemaRustSourceMap {
    pub fn find_by_rust_location(&self, rust_file: &str, line: usize) -> Option<&NemaRustSourceMapEntry> {
        self.entries.iter().find(|e| {
            e.rust_file == rust_file && line >= e.rust_line_range.0 && line <= e.rust_line_range.1
        })
    }
}

/// `.nema → Rust` 降维生成结果
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoweringOutcome {
    pub rust_file: String,
    pub rust_code: String,
    pub ast_hash: String,
    pub source_map: NemaRustSourceMap,
}

/// 降维生成引擎
#[derive(Debug, Default)]
pub struct NemaToRustLowering;

impl NemaToRustLowering {
    /// 将 NEMA 文档结构化降维生成生产级 Rust 源码与双向 SourceMap
    pub fn lower(
        &self,
        nema_file: &str,
        doc: &NemaDocument,
        target_rust_file: &str,
        source_branch: &str,
    ) -> LoweringOutcome {
        let mut rust_code = String::new();
        let mut source_map_entries = Vec::new();
        let mut current_line = 1;

        // 1. 顶层文件级安全守卫与宏约束
        rust_code.push_str("//! Generated from ");
        rust_code.push_str(nema_file);
        rust_code.push_str(" via IDE1.0 Multi-Agent Aggregator Lowering Engine.\n");
        rust_code.push_str("//! DO NOT EDIT MANUALLY - Changes should be authored in .nema specification.\n");
        rust_code.push_str("#![forbid(unsafe_code)]\n\n");
        rust_code.push_str("use serde::{Deserialize, Serialize};\n\n");
        current_line += 6;

        for domain in &doc.domains {
            match domain.mode {
                DomainMode::Table => {
                    // Table 域 -> 生成 Rust Struct 或 Enum
                    for record in &domain.records {
                        let type_name = record
                            .cells
                            .first()
                            .and_then(|c| c.first())
                            .map(|c| c.raw.trim().to_string())
                            .unwrap_or_else(|| format!("EntityDomain{}", domain.id));

                        let start_line = current_line;
                        rust_code.push_str("/// Data entity lowered from NEMA Table Domain \\");
                        rust_code.push_str(&domain.id);
                        rust_code.push('\n');
                        rust_code.push_str("#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]\n");
                        rust_code.push_str(&format!("pub struct {type_name} {{\n"));
                        current_line += 3;

                        // 填充字段
                        for (idx, col) in record.cells.iter().enumerate().skip(1) {
                            let field_val = col.iter().map(|c| c.raw.as_str()).collect::<Vec<_>>().join(" ");
                            let field_name = format!("field_{idx}");
                            rust_code.push_str(&format!("    /// Spec: {field_val}\n"));
                            rust_code.push_str(&format!("    pub {field_name}: String,\n"));
                            current_line += 2;
                        }
                        rust_code.push_str("}\n\n");
                        current_line += 2;
                        let end_line = current_line - 1;

                        let mut hasher = Sha256::new();
                        hasher.update(type_name.as_bytes());
                        let ast_hash = format!("{:x}", hasher.finalize());

                        source_map_entries.push(NemaRustSourceMapEntry {
                            nema_file: nema_file.to_string(),
                            domain_id: domain.id.clone(),
                            domain_mode: domain.mode,
                            record_key: type_name.clone(),
                            nema_span: TextSpan::default(),
                            source_branch: source_branch.to_string(),
                            rust_file: target_rust_file.to_string(),
                            rust_symbol_path: format!("crate::{type_name}"),
                            rust_line_range: (start_line, end_line),
                            generated_rust_ast_hash: ast_hash,
                        });
                    }
                }
                DomainMode::FiveWOneH => {
                    // 5W1H 域 -> 生成 Rust 函数与不变量契约
                    for record in &domain.records {
                        let fn_name = record
                            .cells
                            .first()
                            .and_then(|c| c.first())
                            .map(|c| c.raw.trim().to_string())
                            .unwrap_or_else(|| format!("action_fn_{}", domain.id));

                        let why = record.cells.get(1).map(|c| c.iter().map(|x| x.raw.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                        let how = record.cells.get(2).map(|c| c.iter().map(|x| x.raw.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                        let when = record.cells.get(4).map(|c| c.iter().map(|x| x.raw.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default();

                        let start_line = current_line;
                        rust_code.push_str(&format!("/// [Why] {why}\n"));
                        rust_code.push_str(&format!("/// [How] {how}\n"));
                        rust_code.push_str(&format!("/// [When] {when}\n"));
                        rust_code.push_str(&format!("pub fn {fn_name}() -> Result<(), &'static str> {{\n"));
                        rust_code.push_str("    // Invariant guard generated from NEMA 5W1H specification\n");
                        rust_code.push_str("    Ok(())\n");
                        rust_code.push_str("}\n\n");
                        current_line += 7;
                        let end_line = current_line - 1;

                        let mut hasher = Sha256::new();
                        hasher.update(fn_name.as_bytes());
                        let ast_hash = format!("{:x}", hasher.finalize());

                        source_map_entries.push(NemaRustSourceMapEntry {
                            nema_file: nema_file.to_string(),
                            domain_id: domain.id.clone(),
                            domain_mode: domain.mode,
                            record_key: fn_name.clone(),
                            nema_span: TextSpan::default(),
                            source_branch: source_branch.to_string(),
                            rust_file: target_rust_file.to_string(),
                            rust_symbol_path: format!("crate::{fn_name}"),
                            rust_line_range: (start_line, end_line),
                            generated_rust_ast_hash: ast_hash,
                        });
                    }
                }
                DomainMode::Graph => {
                    // Graph 域 -> 生成模块依赖 use 声明与拓扑状态机
                    let start_line = current_line;
                    rust_code.push_str(&format!("/// State transitions lowered from Graph Domain >{}\n", domain.id));
                    rust_code.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\n");
                    rust_code.push_str(&format!("pub enum StateMachineDomain{} {{\n", domain.id));
                    current_line += 3;

                    for record in &domain.records {
                        if let Some(edge) = &record.graph_edge {
                            rust_code.push_str(&format!("    /// Edge {}: {} -> {}\n", edge.id, edge.from, edge.to));
                            rust_code.push_str(&format!("    Transition{}_{},\n", edge.id, edge.to));
                            current_line += 2;
                        }
                    }
                    rust_code.push_str("}\n\n");
                    current_line += 2;
                    let end_line = current_line - 1;

                    let mut hasher = Sha256::new();
                    hasher.update(domain.id.as_bytes());
                    let ast_hash = format!("{:x}", hasher.finalize());

                    source_map_entries.push(NemaRustSourceMapEntry {
                        nema_file: nema_file.to_string(),
                        domain_id: domain.id.clone(),
                        domain_mode: domain.mode,
                        record_key: format!("StateMachineDomain{}", domain.id),
                        nema_span: TextSpan::default(),
                        source_branch: source_branch.to_string(),
                        rust_file: target_rust_file.to_string(),
                        rust_symbol_path: format!("crate::StateMachineDomain{}", domain.id),
                        rust_line_range: (start_line, end_line),
                        generated_rust_ast_hash: ast_hash,
                    });
                }
            }
        }

        let mut overall_hasher = Sha256::new();
        overall_hasher.update(rust_code.as_bytes());
        let overall_ast_hash = format!("{:x}", overall_hasher.finalize());

        LoweringOutcome {
            rust_file: target_rust_file.to_string(),
            rust_code,
            ast_hash: overall_ast_hash,
            source_map: NemaRustSourceMap {
                entries: source_map_entries,
            },
        }
    }
}

/// AST 漂移与人工代码防静默覆盖守卫 (NemaRustDriftGuard)
#[derive(Debug, Default)]
pub struct NemaRustDriftGuard;

impl NemaRustDriftGuard {
    /// 检查现有 Rust 文件是否已被人类工程师直接手改
    pub fn assert_no_human_drift(
        &self,
        current_disk_content: Option<&str>,
        last_generated_hash: &str,
    ) -> Result<(), String> {
        if let Some(content) = current_disk_content {
            let mut hasher = Sha256::new();
            hasher.update(content.as_bytes());
            let current_hash = format!("{:x}", hasher.finalize());

            if !last_generated_hash.is_empty() && current_hash != last_generated_hash {
                return Err(format!(
                    "HUMAN_EDIT_DRIFT: Rust source has manual edits (disk hash: {} vs last lowered: {}). Aborting overwrite.",
                    &current_hash[..8],
                    &last_generated_hash[..8]
                ));
            }
        }
        Ok(())
    }
}

// ============================================================================
// 阶段 ④：文件级 Git 管理与验证流水线 (Verify Agent)
// ============================================================================

/// Git 分支状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GitBranchState {
    Active,
    ReadyForMerge,
    Merged,
    Conflicted,
}

/// 分支元数据
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitBranchRecord {
    pub branch_name: String,
    pub agent_id: String,
    pub head_commit: String,
    pub state: GitBranchState,
    pub files_touched: Vec<String>,
}

/// 统筹合并 PR 记录
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullRequestRecord {
    pub pr_id: u64,
    pub source_branches: Vec<String>,
    pub target_branch: String,
    pub merge_commit: Option<String>,
    pub status: String,
}

/// 文件级 Git 编排器
#[derive(Debug, Default)]
pub struct GitBranchOrchestrator {
    pub branches: BTreeMap<String, GitBranchRecord>,
    pub next_pr_id: u64,
}

impl GitBranchOrchestrator {
    pub fn new() -> Self {
        let mut orch = Self {
            branches: BTreeMap::new(),
            next_pr_id: 1,
        };
        orch.branches.insert(
            "main".to_string(),
            GitBranchRecord {
                branch_name: "main".to_string(),
                agent_id: "System".to_string(),
                head_commit: "commit-main-baseline".to_string(),
                state: GitBranchState::Active,
                files_touched: Vec::new(),
            },
        );
        orch
    }

    /// 创建或登记特性分支
    pub fn register_branch(&mut self, branch_name: &str, agent_id: &str, files: Vec<String>) {
        self.branches.insert(
            branch_name.to_string(),
            GitBranchRecord {
                branch_name: branch_name.to_string(),
                agent_id: agent_id.to_string(),
                head_commit: format!("commit-{branch_name}-init"),
                state: GitBranchState::Active,
                files_touched: files,
            },
        );
    }

    /// 生成带结构化 Audit Trailer 的 Commit 消息
    pub fn make_commit_message(
        &self,
        title: &str,
        agent_id: &str,
        branch_group: &str,
        nema_hash: &str,
        fencing_token: u64,
    ) -> String {
        format!(
            "{title}\n\nX-IDE-Agent-Id: {agent_id}\nX-IDE-Branch-Group: {branch_group}\nX-IDE-Nema-Hash: {nema_hash}\nX-IDE-Fencing-Token: {fencing_token}"
        )
    }

    /// 发起 PR 请求
    pub fn create_pr(&mut self, source_branches: &[String], target_branch: &str) -> PullRequestRecord {
        let pr = PullRequestRecord {
            pr_id: self.next_pr_id,
            source_branches: source_branches.to_vec(),
            target_branch: target_branch.to_string(),
            merge_commit: None,
            status: "PendingVerify".to_string(),
        };
        self.next_pr_id += 1;
        pr
    }

    /// 验证通过后凭验收证书执行原子 Merge 合并
    pub fn merge_pr(
        &mut self,
        mut pr: PullRequestRecord,
        certificate: &AcceptanceCertificate,
    ) -> Result<PullRequestRecord, String> {
        if certificate.candidate_commit.is_empty() {
            return Err("Cannot merge without valid AcceptanceCertificate".to_string());
        }
        let merge_sha = format!("merge-oid-{}", certificate.epoch);
        pr.merge_commit = Some(merge_sha.clone());
        pr.status = "Merged".to_string();

        if let Some(main_branch) = self.branches.get_mut("main") {
            main_branch.head_commit = merge_sha;
        }
        for b_name in &pr.source_branches {
            if let Some(b) = self.branches.get_mut(b_name) {
                b.state = GitBranchState::Merged;
            }
        }
        Ok(pr)
    }
}

/// 验收合格证书 (AcceptanceCertificate)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AcceptanceCertificate {
    pub certificate_id: String,
    pub epoch: u64,
    pub candidate_commit: String,
    pub verified_by: String,
    pub issued_at: DateTime<Utc>,
    pub signature: String,
}

/// 逆向自愈反馈工单 (IterationFeedbackTicket)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IterationFeedbackTicket {
    pub failed_gate: String,
    pub rust_file: String,
    pub rust_line: usize,
    pub mapped_nema_file: String,
    pub mapped_domain_id: String,
    pub mapped_record_key: String,
    pub responsible_branch: String,
    pub responsible_agent: String,
    pub diagnostic_summary: String,
    pub suggested_task_kind: TaskInputKind,
}

/// 四级验证门禁报告
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyPipelineReport {
    pub epoch: u64,
    pub candidate_commit: String,
    pub build_passed: bool,
    pub tests_passed: bool,
    pub quality_passed: bool,
    pub spec_compliance_passed: bool,
    pub certificate: Option<AcceptanceCertificate>,
    pub feedback_tickets: Vec<IterationFeedbackTicket>,
}

/// 验证智能体流水线执行器
#[derive(Debug, Default)]
pub struct VerifyPipeline;

impl VerifyPipeline {
    /// 执行四道验证门禁；全绿签发证书，否则反查 SourceMap 生成反馈回流工单
    pub fn verify(
        &self,
        epoch: u64,
        candidate_commit: &str,
        build_ok: bool,
        tests_ok: bool,
        quality_ok: bool,
        spec_ok: bool,
        error_locations: &[(String, usize, String)], // (rust_file, line, error_msg)
        source_map: &NemaRustSourceMap,
    ) -> VerifyPipelineReport {
        let all_passed = build_ok && tests_ok && quality_ok && spec_ok;

        if all_passed {
            let mut hasher = Sha256::new();
            hasher.update(format!("{epoch}:{candidate_commit}:PASS").as_bytes());
            let sig = format!("{:x}", hasher.finalize());

            let cert = AcceptanceCertificate {
                certificate_id: format!("CERT-EPOCH-{epoch}"),
                epoch,
                candidate_commit: candidate_commit.to_string(),
                verified_by: "VerifyAgent".to_string(),
                issued_at: Utc::now(),
                signature: sig,
            };

            VerifyPipelineReport {
                epoch,
                candidate_commit: candidate_commit.to_string(),
                build_passed: true,
                tests_passed: true,
                quality_passed: true,
                spec_compliance_passed: true,
                certificate: Some(cert),
                feedback_tickets: Vec::new(),
            }
        } else {
            let mut feedback_tickets = Vec::new();
            for (file, line, diag) in error_locations {
                let mapped = source_map.find_by_rust_location(file, *line);
                let (nema_file, dom_id, rec_key, branch, agent) = match mapped {
                    Some(m) => (
                        m.nema_file.clone(),
                        m.domain_id.clone(),
                        m.record_key.clone(),
                        m.source_branch.clone(),
                        if m.source_branch == "feat/A1" { "Agent-A1" } else { "Agent-A2" }.to_string(),
                    ),
                    None => (
                        "specs/File_A.nema".to_string(),
                        "1".to_string(),
                        "Unknown".to_string(),
                        "feat/A1".to_string(),
                        "Agent-A1".to_string(),
                    ),
                };

                let gate = if !build_ok {
                    "Build"
                } else if !tests_ok {
                    "UnitTest"
                } else if !quality_ok {
                    "Quality"
                } else {
                    "Spec"
                };

                feedback_tickets.push(IterationFeedbackTicket {
                    failed_gate: gate.to_string(),
                    rust_file: file.clone(),
                    rust_line: *line,
                    mapped_nema_file: nema_file,
                    mapped_domain_id: dom_id,
                    mapped_record_key: rec_key,
                    responsible_branch: branch,
                    responsible_agent: agent,
                    diagnostic_summary: diag.clone(),
                    suggested_task_kind: TaskInputKind::DefectRepair,
                });
            }

            VerifyPipelineReport {
                epoch,
                candidate_commit: candidate_commit.to_string(),
                build_passed: build_ok,
                tests_passed: tests_ok,
                quality_passed: quality_ok,
                spec_compliance_passed: spec_ok,
                certificate: None,
                feedback_tickets,
            }
        }
    }
}

// ============================================================================
// 持续优化闭环调度器 (Feedback & Iteration Controller)
// ============================================================================

/// 多智能体流水线统一协调控制器
#[derive(Debug)]
pub struct MultiAgentPipelineCoordinator {
    pub router: RouterEngine,
    pub merger: NemaSemanticMerger,
    pub lowering: NemaToRustLowering,
    pub drift_guard: NemaRustDriftGuard,
    pub git: GitBranchOrchestrator,
    pub verifier: VerifyPipeline,
    pub loop_guard: FeedbackLoopGuard,
}

impl Default for MultiAgentPipelineCoordinator {
    fn default() -> Self {
        Self {
            router: RouterEngine,
            merger: NemaSemanticMerger,
            lowering: NemaToRustLowering,
            drift_guard: NemaRustDriftGuard,
            git: GitBranchOrchestrator::new(),
            verifier: VerifyPipeline,
            loop_guard: FeedbackLoopGuard::default(),
        }
    }
}

impl MultiAgentPipelineCoordinator {
    /// 评估验证反馈，判断是熔断转交人工还是发起下一轮增量迭代
    pub fn process_iteration_feedback(
        &mut self,
        ticket: &IterationFeedbackTicket,
        consumed_tokens: u64,
    ) -> FeedbackDecision {
        let err_fingerprint = format!("{}:{}:{}", ticket.failed_gate, ticket.rust_file, ticket.rust_line);
        self.loop_guard.record_and_check(consumed_tokens, &err_fingerprint)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_router_decomposition_plan_coedit_and_independent() {
        let router = RouterEngine;
        let files = vec![
            "specs/File_A.nema".to_string(),
            "specs/File_B.nema".to_string(),
            "specs/File_C.nema".to_string(),
        ];
        let plan = router.plan("run-001", 1, TaskInputKind::FeatureDevelopment, "Implement Gateway Core", &files);

        assert_eq!(plan.groups.len(), 2);
        match &plan.groups[0] {
            CollaborationGroupKind::CoEditingSameFile { nema_file, assignments, .. } => {
                assert_eq!(nema_file, "specs/File_A.nema");
                assert_eq!(assignments.len(), 2);
                assert_eq!(assignments[0].branch_name, "feat/A1");
                assert_eq!(assignments[1].branch_name, "feat/A2");
            }
            _ => panic!("Expected CoEditingSameFile"),
        }
        match &plan.groups[1] {
            CollaborationGroupKind::IndependentFiles { assignments } => {
                assert_eq!(assignments.len(), 2);
                assert_eq!(assignments[0].branch_name, "feat/B1");
                assert_eq!(assignments[1].branch_name, "feat/B2");
            }
            _ => panic!("Expected IndependentFiles"),
        }
        assert_eq!(plan.dependency_dag.len(), 2);
    }

    #[test]
    fn test_semantic_merger_unifies_dictionary_and_merges_records() {
        let merger = NemaSemanticMerger;
        let base = "\t$1\nTokenZero\n\t\\1\n\\ Type Field\nTokenZero Core\n\n\n";
        let branch_a = "\t$2\nTokenZero\nTokenA\n\t\\1\n\\ Type Field\nTokenZero Core\nTokenA ModuleA\n\n\n";
        let branch_b = "\t$2\nTokenZero\nTokenB\n\t\\1\n\\ Type Field\nTokenZero Core\nTokenB ModuleB\n\n\n";

        let outcome = merger.merge("specs/File_A.nema", base, "feat/A1", branch_a, "feat/A2", branch_b).expect("merge ok");

        assert!(outcome.remapped_dictionary.contains(&"TokenA".to_string()));
        assert!(outcome.remapped_dictionary.contains(&"TokenB".to_string()));
        assert!(outcome.rendered_nema_source.contains("TokenA ModuleA"));
        assert!(outcome.rendered_nema_source.contains("TokenB ModuleB"));
    }

    #[test]
    fn test_lowering_and_source_map_generation() {
        let lowering = NemaToRustLowering;
        let codec = StandardNemaCodec;
        let nema_src = "\t\\1\n\\ Type Field\nGatewayConfig Port\n\t?2\nSendRequest ProcessForwarding InWorker Client Immediate Local\n\n\n";
        let doc = codec.parse(nema_src).expect("parse ok");

        let outcome = lowering.lower("specs/File_A.nema", &doc, "src/a.rs", "feat/A1");

        assert!(outcome.rust_code.contains("pub struct GatewayConfig"));
        assert!(outcome.rust_code.contains("pub fn SendRequest()"));
        assert!(outcome.rust_code.contains("#![forbid(unsafe_code)]"));
        assert_eq!(outcome.source_map.entries.len(), 2);

        // SourceMap 查找
        let found = outcome.source_map.find_by_rust_location("src/a.rs", 12);
        assert!(found.is_some());
    }

    #[test]
    fn test_git_orchestration_and_verify_pipeline_closed_loop() {
        let mut coord = MultiAgentPipelineCoordinator::default();

        // 注册分支
        coord.git.register_branch("feat/A1", "Agent-A1", vec!["specs/File_A.nema".to_string()]);
        coord.git.register_branch("feat/A2", "Agent-A2", vec!["specs/File_A.nema".to_string()]);

        let pr = coord.git.create_pr(&["feat/A1".to_string(), "feat/A2".to_string()], "main");
        assert_eq!(pr.status, "PendingVerify");

        // 1. 模拟全绿通过
        let report_pass = coord.verifier.verify(
            1,
            "commit-cand-001",
            true,
            true,
            true,
            true,
            &[],
            &NemaRustSourceMap::default(),
        );
        assert!(report_pass.certificate.is_some());
        let merged_pr = coord.git.merge_pr(pr, report_pass.certificate.as_ref().unwrap()).expect("merged");
        assert_eq!(merged_pr.status, "Merged");

        // 2. 模拟报错并产生反向自愈工单
        let mut source_map = NemaRustSourceMap::default();
        source_map.entries.push(NemaRustSourceMapEntry {
            nema_file: "specs/File_A.nema".to_string(),
            domain_id: "2".to_string(),
            domain_mode: DomainMode::FiveWOneH,
            record_key: "SendRequest".to_string(),
            nema_span: TextSpan::default(),
            source_branch: "feat/A1".to_string(),
            rust_file: "src/a.rs".to_string(),
            rust_symbol_path: "crate::SendRequest".to_string(),
            rust_line_range: (10, 20),
            generated_rust_ast_hash: "hash".to_string(),
        });

        let report_fail = coord.verifier.verify(
            2,
            "commit-cand-002",
            false, // 编译失败
            false,
            true,
            true,
            &[("src/a.rs".to_string(), 14, "mismatched types".to_string())],
            &source_map,
        );
        assert_eq!(report_fail.feedback_tickets.len(), 1);
        let ticket = &report_fail.feedback_tickets[0];
        assert_eq!(ticket.mapped_nema_file, "specs/File_A.nema");
        assert_eq!(ticket.responsible_branch, "feat/A1");

        // 持续闭环判定
        let decision = coord.process_iteration_feedback(ticket, 5000);
        assert!(matches!(decision, FeedbackDecision::ContinueIteration { next_round: 1 }));
    }
}
