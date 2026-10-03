# Themed Mermaid cases

Diagram types whose colours come from the theme palette, not only node fills. Open under
`theme = "dark"`, `"light"` and `"herdr"` and check each card against the page.

## Sequence with a note

```mermaid
sequenceDiagram
  participant U as User
  participant R as Reader
  U->>R: Open page
  Note over U,R: Notes have their own fill
  R-->>U: Render content
```

## Pie

```mermaid
pie title Pages by status
  "accepted" : 12
  "draft" : 5
  "planned" : 7
```

## Git graph

```mermaid
gitGraph
  commit
  branch feature
  checkout feature
  commit
  checkout main
  merge feature
```
