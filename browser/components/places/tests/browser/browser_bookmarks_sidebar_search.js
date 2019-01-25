/**
 *  Test searching for bookmarks (by title and by tag) from the Bookmarks sidebar.
 */
"use strict";

let sidebar = document.getElementById("sidebar");

const TEST_URI = "http://example.com/";
const BOOKMARKS_COUNT = 4;

async function assertBookmarks(searchValue) {
  let found = 0;

  let searchBox = sidebar.contentDocument.getElementById("search-box");

  ok(searchBox, "search box is in context");

  searchBox.value = searchValue;
  // Call searchBookmarks() directly rather than indirectly through
  // searchBox.doCommand() so we can await its resolution.
  await sidebar.contentWindow.searchBookmarks(searchBox.value);

  let tree = sidebar.contentDocument.getElementById("bookmarks-view");

  for (let i = 0; i < tree.view.rowCount; i++) {
    let cellText = tree.view.getCellText(i, tree.columns.getColumnAt(0));

    if (cellText.includes("example page")) {
      found++;
    }
  }

  info("Reset the search");
  searchBox.value = "";
  searchBox.doCommand();

  is(found, BOOKMARKS_COUNT, "found expected site");
}

add_task(async function test() {
  // Add bookmarks and tags.
  for (let i = 0; i < BOOKMARKS_COUNT; i++) {
    let url = Services.io.newURI(TEST_URI + i);

    await PlacesUtils.bookmarks.insert({
      url,
      title: "example page " + i,
      parentGuid: PlacesUtils.bookmarks.toolbarGuid,
    });
    PlacesUtils.tagging.tagURI(url, ["test"]);
  }

  await withSidebarTree("bookmarks", async function() {
    // Search a bookmark by its title.
    await assertBookmarks("example.com");
    // Search a bookmark by its tag.
    await assertBookmarks("test");
  });

  // Cleanup.
  await PlacesUtils.bookmarks.eraseEverything();
});
