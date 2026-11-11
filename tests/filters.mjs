import assert from 'node:assert/strict';
import fs from 'node:fs';
import vm from 'node:vm';
import { test } from 'node:test';

// Execute the shipped filter function with DOM controls and rows. No browser APIs
// beyond attribute reads, checkbox state and class toggles participate here.
const source = fs.readFileSync(new URL('../src/html/assets.rs', import.meta.url), 'utf8');
const filter = source.slice(source.indexOf('function initFilters(){'), source.indexOf('// ---- lightweight M1'));
function fixture() {
  const control = (attribute, value) => ({ checked: false, getAttribute: name => name === attribute ? value : null });
  const security = ['Tune', 'Engineering', 'constructor'].map(value => control('data-sec', value));
  const tags = ['Fuel Map', 'engine', '__proto__'].map(value => control('data-tag', value));
  const rows = [
    ['Tune', ['Fuel Map']], ['Engineering', ['engine']], [null, []],
    ['constructor', ['__proto__']], ['Tune', ['engine', 'Fuel Map']],
  ].map(([sec, tags]) => {
    const classes = new Set();
    return {
      querySelector: () => ({ getAttribute: name => name === 'data-security' ? sec : JSON.stringify(tags) }),
      classList: { add: value => classes.add(value), remove: value => classes.delete(value) },
      visible: () => !classes.has('filtered'),
    };
  });
  let change;
  const panel = {
    querySelectorAll: query => query === 'input[data-sec]' ? security : tags,
    addEventListener: (name, handler) => { assert.equal(name, 'change'); change = handler; },
  };
  vm.runInNewContext(`${filter}\ninitFilters();`, {
    document: { getElementById: () => panel, querySelectorAll: () => rows },
  });
  return { security, tags, apply: () => { change(); return rows.map(row => row.visible()); } };
}

test('unchecked facets do not hide rows and resetting restores all rows', () => {
  const { security, tags, apply } = fixture();
  assert.deepEqual(apply(), [true, true, true, true, true]);
  tags[0].checked = true;
  assert.deepEqual(apply(), [true, false, false, false, true]);
  security[0].checked = true;
  assert.deepEqual(apply(), [true, false, false, false, true]);
  security[0].checked = false;
  security[1].checked = true;
  assert.deepEqual(apply(), [false, false, false, false, false]);
  tags[1].checked = true;
  assert.deepEqual(apply(), [false, true, false, false, false]);
  for (const control of [...security, ...tags]) control.checked = false;
  assert.deepEqual(apply(), [true, true, true, true, true]);
});

test('tag values retain spaces and prototype names have no implicit membership', () => {
  const { security, tags, apply } = fixture();
  tags[0].checked = true;
  assert.deepEqual(apply(), [true, false, false, false, true]);
  tags[0].checked = false;
  tags[2].checked = true;
  assert.deepEqual(apply(), [false, false, false, true, false]);
  security[0].checked = true;
  assert.deepEqual(apply(), [false, false, false, false, false]);
  security[0].checked = false;
  security[2].checked = true;
  assert.deepEqual(apply(), [false, false, false, true, false]);
});

test('graph communities begin visible and survive two legend toggles', () => {
  const initializer = source.match(/var hidden=.*;/)[0];
  const visibility = source.match(/function vis\(n\)\{[^}]+\}/)[0];
  const toggle = source.match(/hidden\[c\.name\]=!hidden\[c\.name\];/)[0];
  const { visible, click } = vm.runInNewContext(`${initializer}\n${visibility}\n({visible: vis, click: function(c){${toggle}}})`);
  for (const name of ['ordinary', 'constructor', '__proto__', 'toString']) {
    const node = { community: name };
    assert.equal(visible(node), true, name);
    click({ name });
    assert.equal(visible(node), false, name);
    click({ name });
    assert.equal(visible(node), true, name);
  }
});
