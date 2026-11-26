/**
 * @license
 * SPDX-License-Identifier: Apache-2.0
 *
 * Exports usados pela frente H (importação/exportação PBIR de indicadores):
 * `captureBookmark`/`applyBookmark` são puras e não dependem de React/zustand.
 */

export { captureBookmark, applyBookmark, DEFAULT_BOOKMARK_CAPTURE } from './capture';
export type {
  CaptureBookmarkInput,
  CapturedBookmarkFields,
  ApplyBookmarkContext,
  ApplyBookmarkPatch,
  BookmarkCapture,
  BookmarkScope,
} from './capture';
export { ActionButtonBookmarkConfig } from './ActionButtonBookmarkConfig';
export {
  nextBookmarkOrder,
  sortBookmarks,
  createBookmarkId,
  createBookmarkEntry,
  renameBookmarkEntry,
  deleteBookmarkEntry,
  duplicateBookmarkEntry,
  reorderBookmarkEntry,
  setBookmarkGroup,
  ungroupBookmarkEntries,
  updateBookmarkSettings,
} from '../../../store/bookmarks';
