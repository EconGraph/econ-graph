// REQUIREMENT: Polyfills for MSW and Node.js test environment
// PURPOSE: Set up required globals before any other imports
// This file must be imported first in setupTests.ts

// TextEncoder/TextDecoder polyfills for Node.js
if (typeof global.TextEncoder === 'undefined') {
  // eslint-disable-next-line @typescript-eslint/no-var-requires
  const { TextEncoder, TextDecoder } = require('util');
  global.TextEncoder = TextEncoder;
  global.TextDecoder = TextDecoder;
}

// ReadableStream polyfill for MSW
if (typeof global.ReadableStream === 'undefined') {
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { ReadableStream } = require('stream/web');
    global.ReadableStream = ReadableStream;
  } catch (e) {
    // Fallback minimal polyfill
    global.ReadableStream = class ReadableStream {
      // Empty constructor for polyfill compatibility
    };
  }
}

// TransformStream polyfill for MSW
if (typeof global.TransformStream === 'undefined') {
  try {
    // eslint-disable-next-line @typescript-eslint/no-var-requires
    const { TransformStream } = require('stream/web');
    global.TransformStream = TransformStream;
  } catch (e) {
    // Fallback minimal polyfill
    global.TransformStream = class TransformStream {
      // eslint-disable-next-line @typescript-eslint/no-unused-vars
      constructor(_transformer = {}) {
        this.readable = new global.ReadableStream();
        this.writable = {
          write: () => {
            // Mock write method
          },
          close: () => {
            // Mock close method
          },
        };
      }
    };
  }
}

// Minimal Request/Response/Headers polyfills for MSW
if (typeof global.Request === 'undefined') {
  global.Request = class Request {
    constructor(input, init = {}) {
      this.url = input;
      this.method = init.method || 'GET';
      this.headers = new global.Headers(init.headers);
      this.body = init.body;
    }
  };
}

if (typeof global.Response === 'undefined') {
  global.Response = class Response {
    constructor(body, init = {}) {
      this.body = body;
      this.status = init.status || 200;
      this.statusText = init.statusText || 'OK';
      this.headers = new global.Headers(init.headers);
      this.ok = this.status >= 200 && this.status < 300;
    }

    async json() {
      return typeof this.body === 'string' ? JSON.parse(this.body) : this.body;
    }

    async text() {
      return typeof this.body === 'string' ? this.body : JSON.stringify(this.body);
    }
  };
}

if (typeof global.Headers === 'undefined') {
  global.Headers = class Headers {
    constructor(init) {
      this._headers = new Map();
      if (init) {
        if (init instanceof global.Headers) {
          init._headers.forEach((value, key) => this._headers.set(key, value));
        } else if (Array.isArray(init)) {
          init.forEach(([key, value]) => this._headers.set(key.toLowerCase(), value));
        } else if (typeof init === 'object') {
          Object.entries(init).forEach(([key, value]) =>
            this._headers.set(key.toLowerCase(), value)
          );
        }
      }
    }

    get(name) {
      return this._headers.get(name.toLowerCase());
    }

    set(name, value) {
      this._headers.set(name.toLowerCase(), value);
    }

    has(name) {
      return this._headers.has(name.toLowerCase());
    }

    delete(name) {
      this._headers.delete(name.toLowerCase());
    }

    forEach(callback) {
      this._headers.forEach(callback);
    }
  };
}

// Fetch polyfill
if (typeof global.fetch === 'undefined') {
  // eslint-disable-next-line @typescript-eslint/no-unused-vars
  global.fetch = async function (_input, _init) {
    return new global.Response('{}', { status: 200 });
  };
}
