# RFC 0001 Traceability Matrix

The compact fixture set demonstrates interacting rules. The later public conformance suite expands every row into isolated positive and negative cases.

| Pattern | Normative RFC sections | Positive fixture | Negative/diagnostic fixture | Milestone |
| --- | --- | --- | --- | --- |
| P1 fixed choice | 4–7 | `cs_systems` | `cs_systems` incomplete transcript | Audit 1 |
| P2 open pool | 6, 9 | `ee_electives` | `ee_electives` duplicate attempt | Audit 1 |
| P3 alternatives | 6, 12 | `cs_systems` | `cs_systems` inactive branch | Audit 1 |
| P4 credits | 2, 6 | `chemistry_credits` | `chemistry_credits` short credit | Audit 1 |
| P5 caps | 9–10 | `history_sharing` | `history_sharing` total/cap trace | Audit 1 |
| P6 distinct groups | 10 | `applied_math_groups` | `applied_math_groups` repeated area | Audit 1 |
| P7 same group | 10 | `applied_math_groups` | `applied_math_groups` mixed area | Audit 1 |
| P8 sharing | 7, 11 | `history_sharing`, `mbb_sharing` | `history_sharing` forbidden pair | Audit 1 |
| P9 selected subset | 9 | `ee_electives` | `ee_electives` only two advanced | Audit 1 |
| P10 conditional counts | 12–13 | `cs_systems` | `cs_systems` wrong branch count | Audit 1 |
| P11 totals with overlap | 10–11 | `history_sharing` | `history_sharing` distinct total | Audit 1 |
| P12 exclusions | 8 | `ee_electives` | `ee_electives` research exclusion | Audit 1 |
| P13 compound attributes | 8 | `ee_electives` | `ee_electives` predicate rejection | Audit 1 |
| P14 transcript rules | 8, 14 | `psychology_waiver` | `psychology_waiver` CR/D/F attempt | Audit 2 |
| P15 repeats | 7, 14 | `chemistry_credits` | `chemistry_credits` repeat cap | Audit 2 |
| P16 waivers | 13 | `psychology_waiver` | `psychology_waiver` absent fact | Audit 2 |
| P17 roles | 4, 10 | `mbb_sharing` | `mbb_sharing` role total | Audit 1 |
| P18 tracks | 12 | `sociology_track` | `sociology_track` missing declaration | Audit 1 |
| P19 paired contribution | 6 | `chemistry_credits` | `chemistry_credits` one-child pair | Audit 1 |
| P20 approval | 13, 15 | `psychology_waiver` | planning action rejection | Audit 2 |

## Governing requirement coverage

| Requirement area | RFC | Schema definitions | Fixture evidence |
| --- | --- | --- | --- |
| Stable IDs, hierarchy, spans | 3–4 | `semanticNode`, `sourceSpan` | all programs and traces |
| Units, credits, bounds | 2, 5–6 | `contribution`, `bound`, `creditUnits` | Chemistry, History |
| Attempt/canonical identity | 7 | catalog/transcript schemas | History, MB&B |
| Typed predicates/nulls | 8 | `predicate`, institution fields | EE, Psychology |
| Selected-set attachment | 9 | `scope`, `selectedSetConstraint` | EE |
| Group constraints | 10 | `groupConstraint` | Applied Mathematics |
| Sharing and totals | 10–11 | `sharingRule`, `totalConstraint` | History, MB&B |
| Choices and activation | 12 | `choice`, `condition` | CS, Sociology |
| Facts/effects/approvals | 13 | `factRef`, `effect` | Psychology |
| Repeats and ordering | 14 | `repeatConstraint`, `orderingConstraint` | Chemistry, Psychology |
| Audit objective/usefulness | 5, 15 | audit-evaluation schema | CS ambiguous witness |
| Migration | 18 | migration metadata | RFC examples; implementation deferred |

