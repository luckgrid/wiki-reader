# Image fixtures

Manual and snapshot inputs for P3-12b. The cases that must **not** render a picture are
intentional: remote URLs, `..` escapes and missing files all show the text placeholder.

## Local images

A small PNG (20×4 cells at 8×17 px cells):

![Small grid](img/small.png)

A wide PNG that shrinks to the pane width:

![Wide grid](img/wide.png)

A JPEG:

![Wide photo](img/photo.jpg)

A tall PNG, capped at 30 rows, with a white stripe every 100 px to check scroll cropping:

![Tall bands](img/tall.png)

Text after the last image.

## Placeholders

A remote image is never fetched:

![Remote](https://example.com/logo.png)

A path that escapes the collection:

![Escape](../../README.md.png)

A missing file:

![Missing](img/nope.png)

An unsupported format:

![Vector](img/diagram.svg)

## Inline

An image inside a sentence ![inline icon](img/small.png) stays inline text.

- ![In a list](img/small.png)

> ![In a quote](img/small.png)
