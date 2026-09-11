import * as Effect from 'effect/Effect';
import * as effectSdk from '../../packages/provenance/dist/effect.js';

/** Reuse the production-host scenarios through the Effect client interface. */
export function effectModule(module) {
  const execute = async effect => {
    const result = await Effect.runPromise(Effect.match(effect, {
      onSuccess: value => ({ value }), onFailure: error => ({ error }),
    }));
    if ('error' in result) throw result.error;
    return result.value;
  };
  const adapt = client => new Proxy(client, {
    get(target, property) {
      const value = target[property];
      return typeof value === 'function' ? (...args) => execute(value.apply(target, args)) : value;
    },
  });
  return { ...module, ...effectSdk, HttpClient: {
    connect: async (baseUrl, fetch) => adapt(await execute(effectSdk.EffectHttpClient.connect({ baseUrl, fetch }))),
    connectWithBearer: async (baseUrl, bearer, fetch) => adapt(await execute(effectSdk.EffectHttpClient.connect({ baseUrl, bearer, fetch }))),
  } };
}
