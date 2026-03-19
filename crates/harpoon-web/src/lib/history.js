/** Undo/Redo history stack for pipeline editor state. */

const MAX_HISTORY = 50;

export function createHistory(initialState) {
  let stack = [structuredClone(initialState)];
  let index = 0;

  return {
    /** Push a new state snapshot. */
    push(state) {
      // Discard any redo states
      stack = stack.slice(0, index + 1);
      stack.push(structuredClone(state));
      if (stack.length > MAX_HISTORY) {
        stack.shift();
      } else {
        index++;
      }
    },

    /** Undo — returns previous state or null if at start. */
    undo() {
      if (index <= 0) return null;
      index--;
      return structuredClone(stack[index]);
    },

    /** Redo — returns next state or null if at end. */
    redo() {
      if (index >= stack.length - 1) return null;
      index++;
      return structuredClone(stack[index]);
    },

    /** Can undo? */
    get canUndo() { return index > 0; },

    /** Can redo? */
    get canRedo() { return index < stack.length - 1; },

    /** Current history size. */
    get size() { return stack.length; },

    /** Current index. */
    get index() { return index; },
  };
}
