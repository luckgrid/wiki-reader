# Image fixtures

Manual and snapshot inputs for P3-12b/c. The cases that must **not** render a picture are
intentional: remote URLs, `..` escapes and missing files all show the text placeholder.

## Local images

A small PNG (20×4 cells at 8×17 px cells):

![Small grid](img/small.png)

A wide PNG that shrinks to the pane width:

![Wide grid](img/wide.png)

A JPEG:

![Wide photo](img/photo.jpg)

A tall PNG (default max slot height 30 rows, configurable 1–60 via `images.max_slot_rows`), with a white stripe every 100 px to check scroll cropping:

![Tall bands](img/tall.png)

A local SVG (rasterised with `resvg`, no external refs):

![Vector](img/diagram.svg)

Text after the last image.

## Placeholders

A remote image is never fetched:

![Remote](https://example.com/logo.png)

A path that escapes the collection:

![Escape](../../README.md.png)

A missing file:

![Missing](img/nope.png)

An unsupported format:

![Notes](img/notes.txt)

## Inline

An image inside a sentence ![inline icon](img/small.png) stays inline text.

- ![In a list](img/small.png)

> ![In a quote](img/small.png)
