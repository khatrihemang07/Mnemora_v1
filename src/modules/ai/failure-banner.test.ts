import { describe, expect, it } from 'vitest';
import type { Failure } from '../../lib/ipc/bindings';
import failures from '../../lib/ipc/fixtures/ai-failures.json';
import { failureBanner } from './failure-banner';

describe('the failure banners', () => {
  it.each(failures)('gives $kind a banner with actions', ({ kind }) => {
    const banner = failureBanner(kind as Failure);
    expect(banner.title.length).toBeGreaterThan(0);
    expect(banner.actions.length).toBeGreaterThan(0);
  });

  it('offers Try again to offline and interrupted answers, and a new Conversation when too long', () => {
    expect(failureBanner('offline')).toMatchObject({ tone: 'warning', actions: ['retry'] });
    expect(failureBanner('interrupted').actions).toEqual(['retry']);
    expect(failureBanner('too_long')).toMatchObject({
      tone: 'warning',
      actions: ['new_conversation'],
    });
  });

  it('points a model problem at the Model, and a connection problem at the Provider', () => {
    expect(failureBanner('model_unavailable').actions).toEqual(['change_model']);
    expect(failureBanner('unreachable').actions).toEqual(['retry', 'change_provider']);
  });
});
