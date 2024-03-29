/* Any copyright is dedicated to the Public Domain.
 http://creativecommons.org/publicdomain/zero/1.0/ */

"use strict";

// Test toggling ON/OFF the flexbox highlighter on different flex containers from the
// flexbox inspector panel.

const TEST_URI = URL_ROOT + "doc_flexbox_specific_cases.html";

add_task(async function() {
  await addTab(TEST_URI);
  const { inspector, flexboxInspector } = await openLayoutView();
  const { document: doc } = flexboxInspector;
  const { highlighters, store } = inspector;

  const onFlexHighlighterToggleRendered = waitForDOM(doc, "#flexbox-checkbox-toggle");
  await selectNode("#container", inspector);
  const [flexHighlighterToggle] = await onFlexHighlighterToggleRendered;

  info("Checking the #container state of the Flexbox Inspector.");
  ok(flexHighlighterToggle, "The flexbox highlighter toggle is rendered.");
  ok(!flexHighlighterToggle.checked, "The flexbox highlighter toggle is unchecked.");
  ok(!highlighters.flexboxHighlighterShown, "No flexbox highlighter is shown.");

  info("Toggling ON the flexbox highlighter for #container from the layout panel.");
  await toggleHighlighterON(flexHighlighterToggle, highlighters, store);

  info("Checking the flexbox highlighter is created for #container.");
  const highlightedNodeFront = store.getState().flexbox.flexContainer.nodeFront;
  is(highlighters.flexboxHighlighterShown, highlightedNodeFront,
    "Flexbox highlighter is shown for #container.");
  ok(flexHighlighterToggle.checked, "The flexbox highlighter toggle is checked.");

  info("Switching the selected flex container to .container.column");
  const onToggleChange = waitUntilState(store, state => !state.flexbox.highlighted);
  await selectNode(".container.column", inspector);
  await onToggleChange;

  info("Checking the .container.column state of the Flexbox Inspector.");
  ok(!flexHighlighterToggle.checked, "The flexbox highlighter toggle is unchecked.");
  is(highlighters.flexboxHighlighterShown, highlightedNodeFront,
    "Flexbox highlighter is still shown for #container.");

  info("Toggling ON the flexbox highlighter for .container.column from the layout "
    + "panel.");
  await toggleHighlighterON(flexHighlighterToggle, highlighters, store);

  info("Checking the flexbox highlighter is created for .container.column");
  is(highlighters.flexboxHighlighterShown,
    store.getState().flexbox.flexContainer.nodeFront,
    "Flexbox highlighter is shown for .container.column.");
  ok(flexHighlighterToggle.checked, "The flexbox highlighter toggle is checked.");

  await toggleHighlighterOFF(flexHighlighterToggle, highlighters, store);

  info("Checking the flexbox highlighter is not shown.");
  ok(!highlighters.flexboxHighlighterShown, "No flexbox highlighter is shown.");
  ok(!flexHighlighterToggle.checked, "The flexbox highlighter toggle is unchecked.");
});
