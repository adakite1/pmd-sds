const vscode = require('vscode');

const TRIGGER = '`1';
const DECL = /^\s*(?:pub(?:\([^)]*\))?\s+)?(?:(?:const|async|unsafe|extern(?:\s+"[^"]*")?)\s+)*(fn|static|const|struct|enum|union|trait|type|mod|macro_rules!)\s+(?:mut\s+)?(?:r#)?([A-Za-z_]\w*)/;
const IMPL = /^\s*(?:unsafe\s+)?impl\b\s*(?:<[^>]*>\s*)?([^{]*)/;
const SECTION = /\/\*\s*=+\s*(.*?)\s*=+\s*\*\//;
const REQUEST = /\/\*\*\*\s*(.*?)\s*(?:\*\*\*\/|$)/;
const KINDS = {
  fn: ['Function', 'symbol-function'],
  static: ['Variable', 'symbol-variable'],
  const: ['Constant', 'symbol-constant'],
  struct: ['Struct', 'symbol-struct'],
  union: ['Struct', 'symbol-struct'],
  enum: ['Enum', 'symbol-enum'],
  trait: ['Interface', 'symbol-interface'],
  type: ['TypeParameter', 'symbol-type-parameter'],
  mod: ['Module', 'symbol-module'],
  'macro_rules!': ['Function', 'symbol-misc'],
  impl: ['Class', 'symbol-class'],
  request: ['Issue', 'comment'],
  section: ['Folder', 'folder'],
};

const highlight = vscode.window.createTextEditorDecorationType({
  isWholeLine: true,
  backgroundColor: new vscode.ThemeColor('editor.rangeHighlightBackground'),
});

function scan(doc) {
  const out = [];
  let asm = false;
  for (let line = 0; line < doc.lineCount; line++) {
    const text = doc.lineAt(line).text;
    if (asm) {
      if (text.includes('ASM_END)*/')) asm = false;
      continue;
    }
    if (text.includes('/*(ASM_START')) {
      asm = !text.includes('ASM_END)*/');
      continue;
    }
    let m;
    if ((m = SECTION.exec(text))) out.push({ line, kind: 'section', name: m[1] });
    else if ((m = REQUEST.exec(text))) out.push({ line, kind: 'request', name: m[1] || '/*** ***/' });
    else if ((m = DECL.exec(text))) out.push({ line, kind: m[1], name: m[2] });
    else if ((m = IMPL.exec(text))) out.push({ line, kind: 'impl', name: `impl ${m[1].trim()}` });
  }
  return out;
}

function nearest(entries, line) {
  let best = 0;
  entries.forEach((e, i) => { if (e.line <= line) best = i; });
  return best;
}

function reveal(editor, line) {
  const range = new vscode.Range(line, 0, line, 0);
  editor.setDecorations(highlight, [range]);
  editor.revealRange(range, vscode.TextEditorRevealType.InCenterIfOutsideViewport);
}

function go(editor, line) {
  const pos = new vscode.Position(line, editor.document.lineAt(line).firstNonWhitespaceCharacterIndex);
  editor.setDecorations(highlight, []);
  editor.selection = new vscode.Selection(pos, pos);
  editor.revealRange(new vscode.Range(pos, pos), vscode.TextEditorRevealType.InCenterIfOutsideViewport);
}

const completions = {
  provideCompletionItems(doc, pos) {
    const m = /`1\w*$/.exec(doc.lineAt(pos.line).text.slice(0, pos.character));
    if (!m) return;
    const range = new vscode.Range(pos.line, m.index, pos.line, pos.character);
    const entries = scan(doc);
    const near = nearest(entries, pos.line);
    return entries.map((e, i) => {
      const item = new vscode.CompletionItem(
        { label: e.name, description: `${e.kind} :${e.line + 1}` },
        vscode.CompletionItemKind[KINDS[e.kind][0]],
      );
      item.range = range;
      item.insertText = '';
      item.filterText = TRIGGER + e.name;
      item.sortText = String(i).padStart(6, '0');
      item.preselect = i === near;
      item.command = { command: 'snowOutline.jump', title: '', arguments: [e.line] };
      return item;
    });
  },
};

async function jump(line) {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const at = editor.selection.active;
  const next = new vscode.Range(at, at.translate(0, 1));
  if (editor.document.getText(next) === '`') {
    await editor.edit(b => b.delete(next), { undoStopBefore: false, undoStopAfter: false });
  }
  go(editor, line);
}

function pick() {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const view = editor.visibleRanges[0];
  const items = scan(editor.document).map(e => e.kind === 'section'
    ? { label: e.name, kind: vscode.QuickPickItemKind.Separator }
    : { label: e.name, description: `${e.kind} :${e.line + 1}`, iconPath: new vscode.ThemeIcon(KINDS[e.kind][1]), line: e.line });
  const targets = items.filter(i => i.line !== undefined);
  const qp = vscode.window.createQuickPick();
  qp.items = items;
  qp.matchOnDescription = true;
  if (targets.length) qp.activeItems = [targets[nearest(targets, editor.selection.active.line)]];
  let accepted = false;
  qp.onDidChangeActive(([i]) => i && reveal(editor, i.line));
  qp.onDidAccept(() => {
    const [i] = qp.activeItems;
    if (i) { accepted = true; go(editor, i.line); }
    qp.hide();
  });
  qp.onDidHide(() => {
    if (!accepted) {
      editor.setDecorations(highlight, []);
      if (view) editor.revealRange(view, vscode.TextEditorRevealType.AtTop);
    }
    qp.dispose();
  });
  qp.show();
}

async function define() {
  const editor = vscode.window.activeTextEditor;
  if (!editor) return;
  const doc = editor.document;
  const range = editor.selection.isEmpty ? doc.getWordRangeAtPosition(editor.selection.active) : editor.selection;
  const name = range && doc.getText(range).trim().split(/[/:]+/).pop();
  if (!name) return;
  const find = d => scan(d).filter(e => e.kind !== 'section' && e.name === name);
  const here = find(doc);
  if (here.length) {
    const line = editor.selection.active.line;
    return go(editor, (here.find(e => e.line > line) ?? here[0]).line);
  }
  for (const uri of await vscode.workspace.findFiles('**/*.rs', '**/{target,asm-corpus}/**')) {
    if (uri.toString() === doc.uri.toString()) continue;
    const other = await vscode.workspace.openTextDocument(uri);
    const [hit] = find(other);
    if (hit) return go(await vscode.window.showTextDocument(other), hit.line);
  }
  vscode.commands.executeCommand('editor.action.revealDefinition');
}

exports.activate = context => {
  context.subscriptions.push(
    vscode.languages.registerCompletionItemProvider({ language: 'rust' }, completions, '1'),
    vscode.commands.registerCommand('snowOutline.jump', jump),
    vscode.commands.registerCommand('snowOutline.pick', pick),
    vscode.commands.registerCommand('snowOutline.define', define),
  );
};
