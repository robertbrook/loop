---
name: Loop
description: A light ink-blue workbench for exploring the Loop Lisp dialect.
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
typography:
  display: {fontFamily: "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif", fontSize: "clamp(34px, 4vw, 58px)", fontWeight: 650, lineHeight: 1.1, letterSpacing: "-.04em"}
  body: {fontFamily: "ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, Segoe UI, sans-serif"}
  code: {fontFamily: "ui-monospace, SFMono-Regular, Consolas, Liberation Mono, monospace", fontSize: "13px", lineHeight: 1.9}
rounded:
  control: "3px"
components:
  button-primary:
    backgroundColor: "{colors.primary}"
    textColor: "{colors.surface}"
    rounded: "{rounded.control}"
    padding: "10px 13px"
---

# Design System: Loop

## Overview

**Creative North Star: "The Language-Reference Workbench"**
Loop is a light, ink-blue workbench for writing and evaluating programs. Examples sit left, the editor center, and results right; the visual treatment stays clear and tool-like rather than promotional. Cool white surfaces, fine blue-gray dividers, restrained blue accents, and monospace code distinguish reference from editing.
**Key Characteristics:** light cool surfaces; one three-part workbench; squared controls and border-defined panels.

## Colors

Blue is used for actions, selection, and code emphasis; cool neutrals carry the page and panels.
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
- **Display** (650, `clamp(34px, 4vw, 58px)`, 1.1): Main headline. **Title** (600, 24px, 1.25): Guide heading.
- **Body** (400, 16px, 1.7): Introductory copy. **Label** (650, 13px): Panel headings.
- **Code** (400, 13px, 1.9): Editor text and line numbers.

## Layout

Desktop is a bordered three-column grid: 205px examples, flexible editor, and results (260px minimum). At 1100px, the rail and results minimum narrow; at 800px, it becomes one column with horizontally scrolling examples. Masthead and main use 4% side margins, capped at 1500px on wide screens. Intro and guide frame the workbench.

## Elevation & Depth

Flat, with no shadows. Panel boundaries, subtle surface tints, and selected-example fill distinguish regions and state.

## Shapes

Panels and fields are square-edged with thin borders. The primary action has a restrained 3px radius; the rest avoids rounded-card styling. Focus uses an offset outline on buttons, links, and summaries, and an inset outline on text fields.

## Components

### Buttons
- **Primary:** Solid blue, white text, compact padding, 3px radius; hover deepens blue.
- **Quiet:** Borderless muted text; hover shifts to blue and underlines. **Focus:** Offset amber outline.

### Cards / Containers
- **Workbench:** White, thin-bordered three-column workspace; examples use a cool tint. Editor and results use dividers, not shadows.

### Inputs / Fields
- **Editor:** Near-white code surface, monospace text, visible line numbers; focus changes surface to white.
- **REPL:** Transparent input on a bottom rule with a blue prompt. Text fields use an inset blue focus outline.

### Navigation
- **Examples:** Vertical list on desktop; selected item is blue with white text. Narrow screens scroll horizontally.
- **Source link:** Quiet text link, underlined on hover.

### Results
- **Output:** Scrollable monospace values; errors are red. New entries briefly animate unless reduced motion is requested.

## Do's and Don'ts

### Do:
- **Do** keep examples left, editor center, and results right on desktop.
- **Do** use blue and cool neutrals to distinguish actions, content, and surfaces.
- **Do** retain visible keyboard focus and the narrow-screen single-column workbench.

### Don't:
- **Don't** add panel shadows; borders and tonal surfaces provide depth.
- **Don't** turn the workbench into a marketing-style hero.
- **Don't** replace code's monospace treatment with interface typography.
