add_task(async function test() {
  let sidebar = document.getElementById("sidebar");

  // Visited pages listed by descending visit date.
  let pages = [
    "http://sidebar.mozilla.org/a",
    "http://sidebar.mozilla.org/b",
    "http://sidebar.mozilla.org/c",
    "http://www.mozilla.org/d",
  ];

  // Number of pages that will be filtered out by the search.
  const FILTERED_COUNT = 1;

  await PlacesUtils.history.clear();

  // Add some visited page.
  let time = Date.now();
  let places = [];
  for (let i = 0; i < pages.length; i++) {
    places.push({ uri: NetUtil.newURI(pages[i]),
                  visitDate: (time - i) * 1000,
                  transition: PlacesUtils.history.TRANSITION_TYPED });
  }
  await PlacesTestUtils.addVisits(places);

  await withSidebarTree("history", async function() {
    info("Set 'by last visited' view");
    // Call GroupBy() directly rather than indirectly through
    // menuItem.doCommand() so we can await its resolution.
    let menuItem = sidebar.contentDocument.getElementById("bylastvisited");
    await sidebar.contentWindow.GroupBy(menuItem, "lastvisited");
    let tree = sidebar.contentDocument.getElementById("historyTree");
    await tree.initPromise;
    check_tree_order(tree, pages);

    // Set a search value.
    let searchBox = sidebar.contentDocument.getElementById("search-box");
    ok(searchBox, "search box is in context");
    searchBox.value = "sidebar.mozilla";
    // Call searchHistory() directly rather than indirectly through
    // searchBox.doCommand() so we can await its resolution.
    await sidebar.contentWindow.searchHistory(searchBox.value);
    check_tree_order(tree, pages, -FILTERED_COUNT);

    info("Reset the search");
    searchBox.value = "";
    // Call searchHistory() directly rather than indirectly through
    // searchBox.doCommand() so we can await its resolution.
    await sidebar.contentWindow.searchHistory(searchBox.value);
    check_tree_order(tree, pages);
  });

  await PlacesUtils.history.clear();
});

function check_tree_order(tree, pages, aNumberOfRowsDelta = 0) {
  let treeView = tree.view;
  let columns = tree.columns;
  is(columns.count, 1, "There should be only 1 column in the sidebar");

  let found = 0;
  for (let i = 0; i < treeView.rowCount; i++) {
    let node = treeView.nodeForTreeIndex(i);
    // We could inherit delayed visits from previous tests, skip them.
    if (!pages.includes(node.uri))
      continue;
    is(node.uri, pages[i], "Node is in correct position based on its visit date");
    found++;
  }
  is(found, pages.length + aNumberOfRowsDelta, "Found all expected results");
}
