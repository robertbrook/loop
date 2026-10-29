---
name: Loop
description: A cool-blue workbench with light and dark themes for the Loop Lisp dialect.
colors:
  primary: "#2449ba"
  ink: "#19294d"
  muted: "#58667e"
  page: "#f6f8fc"
  surface: "#ffffff"
  editor: "#fcfdff"
  examples: "#edf1fa"
  line: "#d6deed"
  focus: "#d87c25"
  output: "#244395"
  success: "#24774e"
  warning: "#a86a15"
  error: "#aa2739"
  primary-hover: "#18388f"
  quiet-hover: "#e0e7f7"
  dark-primary: "#99b5ff"
  dark-ink: "#dce5f5"
  dark-muted: "#a9b8d2"
  dark-page: "#141b29"
  dark-surface: "#1c2535"
  dark-editor: "#182131"
  dark-line: "#3b4963"
  dark-focus: "#f1b66f"
  dark-output: "#b5caff"
  dark-success: "#79d6a0"
  dark-warning: "#e8ba70"
  dark-error: "#ff9aab"
  dark-primary-text: "#142039"
  dark-quiet-surface: "#27334a"
  dark-quiet-hover: "#344563"
typography:
  title: {fontFamily: "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif", fontSize: "22px", fontWeight: 650}
  body: {fontFamily: "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"}
  code: {fontFamily: "ui-monospace, SFMono-Regular, Consolas, Liberation Mono, monospace", fontSize: "13px", lineHeight: 1.9}
rounded:
  control: "3px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.surface}"
    rounded: "{rounded.control}"
    padding: "8px 16px"
---

# Design System: Loop

## Overview

**Creative North Star: "The Language-Reference Workbench"**
Loop is a cool-blue workbench for writing and evaluating programs, with light and dark themes. The editor sits left and results right, with examples in a compact select. Static introductions, tutorials, captions, and decorative copy are omitted; language documentation lives in the repository reference.
**Key Characteristics:** cool surfaces; one two-part workbench; squared controls and border-defined panels.

## Colors

Blue is used for actions, selection, and code emphasis; cool neutrals carry the page and panels.
CSS `light-dark()` tokens and `color-scheme` cover every surface, control, status, focus ring, and output color. System is the default; the header select can force Light or Dark, persisted locally before the stylesheet loads. Dark mode uses slate-blue surfaces and pale blue accents, without changing layout or adding effects.
### Primary
- **Ink Blue** (`{colors.primary}`): Actions, selected example, and interactive emphasis. **Code Blue** (`{colors.output}`): Lisp text and evaluated values.
### Neutral
- **Deep Ink** (`{colors.ink}`): Foreground and REPL input. **Muted Slate** (`{colors.muted}`): Supporting copy and labels.
- **Cool Page** (`{colors.page}`): Page background. **White Surface** (`{colors.surface}`): Workbench and panels.
- **Editor White** (`{colors.editor}`): Resting editor. **Example Tint** (`{colors.examples}`): Example navigation and quiet REPL action.
- **Divider Blue-Gray** (`{colors.line}`): Panel boundaries. **Focus Amber** (`{colors.focus}`): Keyboard focus.
- **Ready Green** (`{colors.success}`): Runtime-ready status. **Loading Amber** (`{colors.warning}`): Loading status. **Error Red** (`{colors.error}`): Errors.

## Typography

**Display Font:** System sans-serif stack  
**Body Font:** System sans-serif stack  
**Label/Mono Font:** System monospace stack
**Character:** Compact interface labels and readable explanatory copy pair with evenly spaced monospace code.
### Hierarchy
- **Title** (650, 22px): Product name. **Label** (650, 13px): Panel headings.
- **Code** (400, 13px, 1.9): Editor text and line numbers.

## Layout

Desktop is a bordered two-column grid: editor and results in a 1.3:1 ratio. At 800px, it becomes one column. Masthead and main use a 94% width capped at 1500px. A compact header contains the product name, runtime status, reference, and source links.

## Elevation & Depth

Flat, with no shadows. Panel boundaries, subtle surface tints, and selected-example fill distinguish regions and state.

## Shapes

Panels and fields are square-edged with thin borders. The primary action has a restrained 3px radius; the rest avoids rounded-card styling. Focus uses an offset outline on buttons, links, and summaries, and an inset outline on text fields.

## Components

### Buttons
- **Primary:** Solid blue, white text, compact padding, 3px radius; hover deepens blue.
- **Quiet:** Borderless muted text; hover shifts to blue and underlines. **Focus:** Offset amber outline.

### Cards / Containers
- **Workbench:** White, thin-bordered two-column workspace. Editor and results use dividers, not shadows.

### Inputs / Fields
- **Editor:** Near-white code surface, monospace text, visible line numbers; focus changes surface to white.
- **REPL:** Transparent input on a bottom rule with a blue prompt. Text fields use an inset blue focus outline.

### Navigation
- **Examples:** Native select above the editor.
- **Source link:** Quiet text link, underlined on hover.

### Results
- **Output:** Scrollable monospace values; errors are red. New entries briefly animate unless reduced motion is requested.

## Do's and Don'ts

### Do:
- **Do** keep the program and results side by side on desktop.
- **Do** use blue and cool neutrals to distinguish actions, content, and surfaces.
- **Do** retain visible keyboard focus and the narrow-screen single-column workbench.

### Don't:
- **Don't** add panel shadows; borders and tonal surfaces provide depth.
- **Don't** turn the workbench into a marketing-style hero.
- **Don't** add static tutorials or promotional copy to the workbench.
- **Don't** replace code's monospace treatment with interface typography.
