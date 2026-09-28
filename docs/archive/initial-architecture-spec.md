---
class: spec
status: abandoned
scope: [specengine]
ref: superseded by docs/specs/specengine-platform (review in 03-critique-of-initial-spec.md)
---

# Technical Specification: SpecEngine Platform

**An Atomic Specification, Context-Engine, and Sub-Agent Orchestration Platform for AI-Assisted Software Engineering**

---

## 1. Executive Summary & Core Principles

**SpecEngine** is a Dockerized, multi-tenant, specification-first development platform designed to eliminate **Context Collapse** in large-scale LLM-driven software projects (such as game engines in Rust + Bevy, complex backend systems, or full-stack web applications).

### 1.1 Guiding Principles

1. **Decoupled Responsibilities:** Specifications define *WHAT* and *WHY* (requirements, domain invariants, edge cases, contracts). Implementation code defines *HOW* and *WHERE* (AST symbols, file structures, execution patterns). Specs contain zero hardcoded file paths.
2. **Atomic Node Granularity:** Documents are structured as hierarchical trees of individually versioned nodes (`SpecNodes`) rather than monolithic files, avoiding massive token overhead and false-positive version invalidations.
3. **Symbol-Level Binding:** Requirements are bound directly to Abstract Syntax Tree (AST) symbols (`fn`, `struct`, `system`, `enum`), rather than raw file lines or physical files.
4. **Sub-Agent Alignment & Code Discovery:** Sub-agents operate via a structured 3-phase execution pipeline (**Discovery** $\rightarrow$ **Planning** $\rightarrow$ **Execution & Bottom-Up Binding**).
5. **Multi-User Real-Time Sync:** Supports concurrent human and AI edits via WebSockets, CRDT-based state updates, and optional Git mirroring.

---

## 2. System Architecture & Topology

SpecEngine is deployed as a single, containerized stack housing the web interface, reactive backend API, Tree-sitter AST parsing engine, and an embedded Model Context Protocol (MCP) server.

```
┌──────────────────────────────────────────────────────────────────────────────┐
│                            DOCKER CONTAINER STACK                            │
│                                                                              │
│  ┌────────────────────────┐  WebSocket / SSE  ┌───────────────────────────┐  │
│  │   Frontend Web UI      │◄─────────────────►│   Backend API (Axum/Rust) │  │
│  │ (SvelteKit / React)    │                   │   & State Engine          │  │
│  └────────────────────────┘                   └─────────────┬─────────────┘  │
│                                                             │                │
│  ┌────────────────────────┐  stdio / SSE (JSON-RPC) ┌───────┴─────────────┐  │
│  │  CLI / Dev Tools       │◄───────────────────────►│   MCP Server        │  │
│  └────────────────────────┘                         └───────┬─────────────┘  │
│                                                             │                │
│  ┌────────────────────────┐                         ┌───────┴─────────────┐  │
│  │ SQLite / PostgreSQL    │◄────────────────────────┤ Tree-sitter Engine  │  │
│  │ + Vector / LTree Ext   │                         │ (AST Subtree Hashes)│  │
│  └────────────────────────┘                         └─────────────────────┘  │
└──────────────────────────────────────────────────────────────────────────────┘
```

### 2.1 Technology Stack

* **Backend Engine:** Rust (Axum framework) for zero-cost abstractions, high-concurrency WebSocket broadcast, and native `tree-sitter` bindings.
* **Database Layer:** PostgreSQL with `ltree` and `pgvector` extensions (or embedded SQLite with `fts5` and vector extensions for local single-command deployment).
* **Frontend Web UI:** SvelteKit or React with Tailwind CSS, TipTap / Monaco editor, and real-time WebSocket state handlers.
* **AST Parsing Engine:** `tree-sitter` multi-language parser (supporting Rust, C++, TypeScript, Go, Python).
* **AI Protocol:** Model Context Protocol (MCP) over stdio or HTTP/SSE.

---

## 3. Data Model & Database Schema

The platform combines relational graph structures for strict hierarchy with vector embeddings for semantic search across drafts and global patterns.

```
                     ┌───────────────────┐
                     │     Projects      │
                     └─────────┬─────────┘
                               │ 1
                               │
                               │ N
                     ┌─────────┴─────────┐
                     │    SpecNodes      │◄───────┐ (Self-Referential
                     └─────────┬─────────┘        │  Parent-Child)
                               │ 1                │
                               │                  │
                               │ N                │
                     ┌─────────┴─────────┐        │
                     │   CodeBindings    ├────────┘
                     └───────────────────┘
```

### 3.1 DDL Schema Definition

```sql
-- Projects Isolation
CREATE TABLE projects (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL,
    slug TEXT UNIQUE NOT NULL,
    description TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Global Reusable Architectural Patterns & Snippets
CREATE TABLE global_library_nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    category TEXT NOT NULL CHECK(category IN ('Pattern', 'Architecture', 'Glossary', 'Rule')),
    tags TEXT[]
);

-- Atomic Specification Nodes
CREATE TABLE spec_nodes (
    id TEXT PRIMARY KEY, -- Human-readable ID (e.g., REQ-STAM-001)
    project_id UUID REFERENCES projects(id) ON DELETE CASCADE,
    parent_node_id TEXT REFERENCES spec_nodes(id),
    path TEXT NOT NULL, -- LTree path representation (e.g., 'ProjectA.Gameplay.Stamina')
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    version TEXT NOT NULL DEFAULT '1.0.0',
    content_hash TEXT NOT NULL, -- SHA-256 computed strictly over node content
    status TEXT NOT NULL CHECK(status IN (
        'Draft', 
        'Pending_User_Approval', 
        'Approved', 
        'Implemented', 
        'Verified', 
        'Drift_Detected',
        'Rejected'
    )),
    origin_library_id UUID REFERENCES global_library_nodes(id),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Symbol-Level Implementation Links
CREATE TABLE code_bindings (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    spec_node_id TEXT REFERENCES spec_nodes(id) ON DELETE CASCADE,
    repository_uri TEXT NOT NULL,
    file_path TEXT NOT NULL,
    symbol_type TEXT NOT NULL CHECK(symbol_type IN ('function', 'struct', 'enum', 'impl_block', 'system', 'component')),
    symbol_name TEXT NOT NULL, -- e.g., 'game::systems::stamina::regen_system'
    ast_hash TEXT NOT NULL,    -- SHA-256 of canonical Tree-sitter AST subtree
    last_sync_commit TEXT NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);

-- Revisions & Diff History
CREATE TABLE node_revisions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    spec_node_id TEXT REFERENCES spec_nodes(id) ON DELETE CASCADE,
    version TEXT NOT NULL,
    diff_content TEXT NOT NULL,
    content_snapshot TEXT NOT NULL,
    author_type TEXT NOT NULL CHECK(author_type IN ('User', 'AI_Agent')),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP
);
```

---

## 4. Sub-Agent Execution Pipeline & Code Navigation Protocol

High-level specifications contain **no file-path assumptions**. Sub-agents handle code navigation, implementation planning, and AST symbol mapping autonomously via a 3-phase execution engine.

```
┌────────────────────────────────────────────────────────────────────────┐
│                     PHASE 1: DISCOVERY & REPO MAP                      │
│ Sub-agent reads SpecNode -> Builds AST Repo Map -> Locates Target AST  │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   PHASE 2: PLANNING (Plan Spec)                        │
│ Sub-agent drafts Implementation Plan -> Validates Edge Cases           │
└──────────────────────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
┌────────────────────────────────────────────────────────────────────────┐
│              PHASE 3: EXECUTION, TESTING & AST BINDING                 │
│ Writes Code -> Generates Unit Tests -> Registers AST via MCP Tool      │
└────────────────────────────────────────────────────────────────────────┘
```

### 4.1 Phase 1: Discovery & Local AST Navigation

When a sub-agent receives a task from `SpecEngine` (e.g., `REQ-STAM-001`):
1. **Spec Retrieval:** The sub-agent fetches the targeted `Context Bundle` via MCP (`get_context_bundle`).
2. **Repo Map Construction:** The agent queries local environment tools or `tree-sitter` to build a lightweight symbol map of the codebase (list of all `struct`, `fn`, `enum`, `impl` signatures).
3. **Symbol Matching:** The agent matches requirements against existing symbols (e.g., locating `struct Stamina` in `src/components/stamina.rs` and `fn regen_system` in `src/systems/stamina.rs`).

### 4.2 Phase 2: Implementation Planning

Before modifying code, the sub-agent generates an **Implementation Plan**:
* Identifies target files and symbols to be created or updated.
* Maps spec edge cases directly to code logic.
* If logical contradictions or ambiguities are found in the spec during planning, the sub-agent invokes `propose_node_update` via MCP to request human clarification rather than making unverified assumptions.

### 4.3 Phase 3: Execution, Unit Test Generation & Bottom-Up Binding

1. **Implementation:** Sub-agent writes domain-compliant code (e.g., pure Rust functions + Bevy Systems).
2. **Edge-Case Validation (Test Generation):** Sub-agent MUST generate isolated unit tests (e.g., `#[test]` in Rust) directly validating edge cases described in the spec node.
3. **Bottom-Up Symbol Registration:** Upon success, the sub-agent executes `bind_code_symbol`:
   ```json
   {
     "node_id": "REQ-STAM-001",
     "file_path": "src/systems/stamina.rs",
     "symbol_type": "system",
     "symbol_identifier": "game::systems::stamina::regen_system"
   }
   ```
4. **Historical Memory:** Future sub-agent runs targeting `REQ-STAM-001` automatically receive pre-bound symbol paths from `SpecEngine`, bypassing Phase 1 discovery.

---

## 5. AST Symbol-Level Drift Engine

To eliminate false-positive version invalidations when unrelated code in the same file changes, SpecEngine hashes **AST Subtrees** instead of physical file hashes.

### 5.1 AST Subtree Hashing Algorithm

1. The developer or AI agent marks a symbol: `@implements REQ-STAM-001`.
2. When a file changes, `tree-sitter` parses the file into an AST.
3. The engine extracts the specific symbol node (e.g., `FunctionItem` named `regen_system`).
4. Comments, docstrings, and formatting whitespace are stripped from the subtree tokens.
5. Hash computation: 
$$\text{Hash}_{\text{AST}} = \text{SHA256}(\text{CanonicalAST}(\text{Symbol}))$$

```
[ Code Change in File ] ──► [ Tree-sitter Parse ] ──► [ Extract Symbol Subtree ]
                                                              │
                                                              ▼
[ No Change in Target AST ] ◄── [ Compare Hashes ] ◄── [ Strip Comments/Space ]
          │                                                   │
          ▼                                                   ▼
  (Status: VERIFIED)                                   [ Compute SHA-256 ]
```

### 5.2 Drift Detection Matrix

| Spec Node Hash Changed? | Code Symbol AST Hash Changed? | System Status | Triggered Action |
| :--- | :--- | :--- | :--- |
| **No** | **No** | `Verified` | No action required. |
| **Yes** | **No** | `Approved` (Spec ahead) | Signal sent to AI Agent to update code implementation. |
| **No** | **Yes** | `Drift_Detected` (Code ahead) | Web UI flags node. Prompts user: "Accept Code Changes" or "Reject Implementation". |
| **Yes** | **Yes** | `Conflict` | Requires manual resolution via Web UI Diff Inspector. |

---

## 6. Human-in-the-Loop & Multi-User Collaboration

SpecEngine provides multi-user real-time collaboration using WebSockets and CRDTs (Conflict-free Replicated Data Types) for state changes.

```
                       ┌──────────┐
                       │  Draft   │
                       └────┬─────┘
                            │ AI or User Edits
                            ▼
               ┌──────────────────────────┐
               │  Pending_User_Approval   │
               └────────────┬─────────────┘
                            │
            ┌───────────────┴───────────────┐
            │ User Approves                 │ User Rejects
            ▼                               ▼
     ┌──────────────┐                ┌──────────────┐
     │   Approved   │                │   Rejected   │
     └──────┬───────┘                └──────────────┘
            │ Code Agent Implements
            ▼
    ┌────────────────┐
    │  Implemented   │
    └───────┬────────┘
            │ AST Sync Engine Verifies
            ▼
     ┌──────────────┐    AST Code Change    ┌────────────────┐
     │   Verified   ├──────────────────────►│ Drift_Detected │
     └──────────────┘                       └────────────────┘
```

### 6.1 Multi-User Synchronization Models

* **Model A: Centralized Real-Time (Server Container):** Hosted on a shared server. Users edit documents concurrently using CRDTs (`Yjs`/`Automerge`). Approvals are broadcast live over WebSockets.
* **Model B: Git Mirroring (Docs-as-Code):** Approved spec nodes automatically mirror to a `.spec/` directory inside the Git repository, allowing offline work and versioning along feature branches.

---

## 7. Model Context Protocol (MCP) Interface

The embedded MCP server provides standard resources and tools for AI clients (Claude Code, Cursor, Custom Sub-agents).

### 7.1 MCP Resources

* `spec://projects/{project_slug}/tree` — Returns lightweight structural tree (IDs, Titles, Statuses, LTree Paths). Excludes text bodies to conserve tokens.
* `spec://nodes/{node_id}` — Returns full text, version history, and symbol bindings for a specific node.
* `spec://tasks/backlog?project={project_slug}` — Returns all `Approved` nodes awaiting implementation.

### 7.2 MCP Tools

#### `get_context_bundle`
* **Description:** Compiles a minimal Markdown context package containing target requirements, parent rules, and bound AST signatures.
* **Input Schema:**
  ```json
  {
    "type": "object",
    "properties": {
      "target_node_ids": { "type": "array", "items": { "type": "string" } },
      "include_parent_chain": { "type": "boolean", "default": true }
    },
    "required": ["target_node_ids"]
  }
  ```

#### `propose_spec_node`
* **Description:** Proposes creating a new specification node.
* **Input Schema:**
  ```json
  {
    "type": "object",
    "properties": {
      "project_slug": { "type": "string" },
      "parent_node_id": { "type": "string" },
      "title": { "type": "string" },
      "content": { "type": "string" }
    },
    "required": ["project_slug", "title", "content"]
  }
  ```

#### `propose_node_update`
* **Description:** Proposes updates to an existing node. Sets node status to `Pending_User_Approval`.
* **Input Schema:**
  ```json
  {
    "type": "object",
    "properties": {
      "node_id": { "type": "string" },
      "title": { "type": "string" },
      "content": { "type": "string" }
    },
    "required": ["node_id", "content"]
  }
  ```

#### `bind_code_symbol`
* **Description:** Binds a code symbol (AST subtree) to a specification node.
* **Input Schema:**
  ```json
  {
    "type": "object",
    "properties": {
      "node_id": { "type": "string" },
      "repository_uri": { "type": "string" },
      "file_path": { "type": "string" },
      "symbol_type": { "type": "string", "enum": ["function", "struct", "enum", "impl_block", "system", "component"] },
      "symbol_identifier": { "type": "string" }
    },
    "required": ["node_id", "file_path", "symbol_identifier"]
  }
  ```

#### `report_implementation_status`
* **Description:** Reports execution status and test results from sub-agent runs.
* **Input Schema:**
  ```json
  {
    "type": "object",
    "properties": {
      "node_id": { "type": "string" },
      "commit_sha": { "type": "string" },
      "test_passed": { "type": "boolean" },
      "status": { "type": "string", "enum": ["Implemented", "Failed"] }
    },
    "required": ["node_id", "commit_sha", "test_passed", "status"]
  }
  ```

---

## 8. Domain Layering Framework (Rust + Bevy Game Engine)

Specifications are organized into a 4-layer domain matrix to separate pure logic from Bevy ECS execution details.

| Layer | Specification Scope | Rust / Bevy Implementation |
| :--- | :--- | :--- |
| **Domain Data** | Entity state, attributes, resource types. | `Component`, `Resource` (`struct Stamina { current: f32, max: f32 }`) |
| **Domain Rules** | Pure math, state formulas, invariants. | Pure Rust methods/functions (no `World` or `Query` dependencies) |
| **Systems** | Frame loops, processes, scheduling. | Bevy `System` (`fn stamina_regen_system(...)`) |
| **Events** | State transition triggers, messages. | Bevy `Event` / `EventReader` / `EventWriter` |

### 8.1 Sample Spec Node Format

```markdown
---
id: REQ-BEVY-STAM-01
title: Stamina Regeneration Mechanics
layer: Domain_Rule
status: Approved
version: 1.1.0
bindings:
  - file: src/systems/stamina.rs
    symbol: game::systems::stamina::regen_system
---

# Stamina Regeneration Mechanics

## 1. Description
When an entity is not sprinting, stamina regenerates over time up to `max_stamina`.

## 2. Invariants & Edge Cases
* Base regeneration rate: $10.0 \text{ units/sec}$.
* Regeneration delay after sprinting: $1.5 \text{ seconds}$.
* If entity has `Exhausted` status effect, regeneration rate is reduced by $50\%$.
* **Edge Case:** If stamina reaches $0.0$, apply `Exhausted` event immediately.

## 3. Associated Symbols
* `@implements REQ-BEVY-STAM-01` in function `regen_system`.
```

---

## 9. Docker Deployment Configuration

SpecEngine runs as a single dockerized environment.

### 9.1 `docker-compose.yml`

```yaml
version: '3.8'

services:
  specengine:
    build:
      context: .
      dockerfile: Dockerfile
    container_name: specengine_app
    ports:
      - "8080:8080"   # Web UI & REST API
      - "3000:3000"   # MCP Server (stdio / SSE)
    environment:
      - DATABASE_URL=sqlite:///app/data/specengine.db
      - RUST_LOG=info
    volumes:
      - spec_data:/app/data
      - ../:/app/repo_target # Mounted codebase for local AST parsing

volumes:
  spec_data:
```

---

## 10. Verification & Acceptance Criteria

1. **Token Efficiency:** Context packages generated via `get_context_bundle` must consume $< 2,000$ tokens for standard isolated tasks.
2. **WebSocket Synchronization:** UI updates triggered by MCP tool executions must render on the Web UI within $< 100\text{ ms}$.
3. **AST Drift Isolation:** Modifying comments, whitespace, or unrelated functions within a source file must **not** trigger `Drift_Detected` on bound symbols.
4. **Sub-Agent Compliance:** Sub-agents must generate passing unit tests (`#[test]`) for specified edge cases before reporting `Implemented` status.