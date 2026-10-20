import { createAppRouter } from '../lib/router';
import { modules } from './registry';

export const router = createAppRouter(modules);
