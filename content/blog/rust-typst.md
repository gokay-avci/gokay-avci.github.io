+++
title = "Scientific Typesetting with Typst"
date = 2025-02-10
description = "How to integrate Typst formulas into a Zola site."
[taxonomies]
tags = ["Typst", "Tools"]
categories = ["Journal"]
+++

**Typst** is a new markup-based typesetting system that is much faster than LaTeX.

### Example Embed
{% typst() %}
<svg width="200" height="50" viewBox="0 0 200 50" xmlns="http://www.w3.org/2000/svg">
  <text x="10" y="30" font-family="serif" font-size="20">F = ma</text>
</svg>
{% end %}