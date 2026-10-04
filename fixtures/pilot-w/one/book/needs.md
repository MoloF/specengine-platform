---
sort: register
Stage: Settled
---

# Needs

The notebook opens with the needs the engine must meet.

| Code | Wording | Weight | Area |
|------|---------|--------|------|
| NEED-01 | The engine reads a pipe \| inside a cell. | high | input |
| NEED-02 | The engine keeps every byte it reads. | low | store |
| OLDR-05 | A need still cited under its old prefix. | high | store |

Prose after the table names no record.

- [x] **ASK-012: Short `name`**: Which name does the notebook print?
- [ ] **ASK-013**: Does the engine stop on the first error?
- an ID-less sibling item stays where it is

## Rules

The rules below become sections of this document.

- [ ] **RULE-01: First rule**: The engine reads the notebook before the flows.
- **RULE-02**: The engine writes nothing outside its scratch.

A paragraph between the rules and the subsection.

### Gate {#NEED-40 level=two}

The gate text moves to a record file.

It has two paragraphs; the [login flow](flows/login.md) passes it under
the plan NEED-30, see [[Gate History]].

## Old law {#LAW-09}

A section defined under a legacy prefix stays in place.

## Duplicates

| Code | Wording |
|------|---------|
| NEED-02 | A second definition of an existing need. |
