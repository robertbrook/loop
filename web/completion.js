const syntax = [
  "and", "begin", "case", "cond", "define", "defmacro", "do", "else", "export", "if",
  "import", "lambda", "let", "let*", "letrec", "load", "module", "or", "quote",
  "quasiquote", "set!", "unless", "unquote", "unquote-splicing", "when", "=>",
];
const delimiter = /[\s()"'`,;]/u;

export function completionToken(source, start, end = start) {
  if (start !== end) return null;
  let string = false, comment = false, escaped = false;
  for (let i = 0; i < start; i++) {
    const char = source[i];
    if (comment) {
      if (char === "\n") comment = false;
    } else if (string) {
      if (escaped) escaped = false;
      else if (char === "\\") escaped = true;
      else if (char === '"') string = false;
    } else if (char === ";") comment = true;
    else if (char === '"') string = true;
  }
  if (string || comment) return null;
  let from = start, to = start;
  while (from > 0 && !delimiter.test(source[from - 1])) from--;
  while (to < source.length && !delimiter.test(source[to])) to++;
  const prefix = source.slice(from, start);
  if (!prefix || /^#|^[+-]?(?:\d|\.\d)/u.test(prefix)) return null;
  return { from, to, prefix };
}

export function attachCompletion(input, list) {
  let names = [], matches = [], token, active = 0, composing = false;
  function close() {
    matches = [];
    list.hidden = true;
    list.replaceChildren();
    input.setAttribute("aria-expanded", "false");
    input.removeAttribute("aria-activedescendant");
  }
  function accept(index) {
    const current = completionToken(input.value, input.selectionStart, input.selectionEnd);
    if (!current || !token || current.from !== token.from || current.prefix !== token.prefix) return close();
    input.setRangeText(matches[index], current.from, current.to, "end");
    close();
    input.focus();
  }
  function render() {
    list.replaceChildren();
    matches.forEach((name, index) => {
      const option = document.createElement("li");
      option.className = `list-group-item list-group-item-action font-monospace text-break${index === active ? " active theme-primary" : ""}`;
      option.id = `repl-option-${index}`;
      option.setAttribute("role", "option");
      option.setAttribute("aria-selected", String(index === active));
      option.textContent = name;
      option.addEventListener("pointerdown", event => event.preventDefault());
      option.addEventListener("click", () => accept(index));
      list.append(option);
    });
    list.hidden = false;
    input.setAttribute("aria-expanded", "true");
    input.setAttribute("aria-activedescendant", `repl-option-${active}`);
    list.children[active].scrollIntoView?.({ block: "nearest" });
  }
  function update() {
    close();
    if (composing || document.activeElement !== input) return;
    token = completionToken(input.value, input.selectionStart, input.selectionEnd);
    if (!token) return;
    matches = names.filter(name => name.startsWith(token.prefix)).slice(0, 8);
    if (matches.length === 1 && matches[0] === token.prefix) return close();
    if (!matches.length) return;
    active = 0;
    render();
  }
  input.addEventListener("input", update);
  input.addEventListener("click", update);
  input.addEventListener("blur", close);
  input.addEventListener("compositionstart", () => { composing = true; close(); });
  input.addEventListener("compositionend", () => { composing = false; update(); });
  input.addEventListener("keyup", event => {
    if (["ArrowLeft", "ArrowRight", "Home", "End"].includes(event.key)) update();
  });
  return {
    close,
    setSymbols(symbols) {
      names = [...new Set([...syntax, ...symbols])].sort();
      update();
    },
    handleKey(event) {
      if (event.isComposing || composing || event.ctrlKey || event.metaKey || event.altKey || !matches.length) return false;
      if (event.key === "Escape") close();
      else if ((event.key === "Tab" && !event.shiftKey) || event.key === "Enter") accept(active);
      else if (event.key === "ArrowDown" || event.key === "ArrowUp") {
        active = (active + (event.key === "ArrowDown" ? 1 : matches.length - 1)) % matches.length;
        render();
      } else return false;
      event.preventDefault();
      return true;
    },
  };
}
