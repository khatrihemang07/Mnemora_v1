import { describe, expect, it } from 'vitest';
import type { TodoTaskDeletedEvent } from '../../lib/ipc/bindings';
import { followRemoteDelete, remoteDeletedMessage } from './remote-delete';

const event: TodoTaskDeletedEvent = {
  task_id: 'hike',
  title: 'Plan weekend hike',
  device: 'Pixel 8',
};

describe('a Task deleted on another device', () => {
  it('names the Task and the device', () => {
    expect(remoteDeletedMessage(event)).toBe('‘Plan weekend hike’ was deleted on Pixel 8');
    expect(remoteDeletedMessage({ ...event, device: null })).toBe(
      '‘Plan weekend hike’ was deleted on another device',
    );
  });

  it('closes the sheet only when that Task is the open one', () => {
    expect(followRemoteDelete('hike', event)).toEqual({
      message: '‘Plan weekend hike’ was deleted on Pixel 8',
    });
    expect(followRemoteDelete('other', event)).toBeNull();
    expect(followRemoteDelete(null, event)).toBeNull();
  });
});
