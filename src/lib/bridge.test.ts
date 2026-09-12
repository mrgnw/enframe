import { describe, expect, it } from 'vitest';
import { isLocalInstance, normalizeConnection } from './bridge';

describe('normalizeConnection', () => {
	it('gives a bare host this app’s port, not the web’s', () => {
		expect(normalizeConnection('192.168.1.9')).toBe('http://192.168.1.9:5911');
		expect(normalizeConnection('desktop.example.ts.net')).toBe('http://desktop.example.ts.net:5911');
	});

	it('keeps a port the user typed', () => {
		expect(normalizeConnection('100.64.0.7:5912')).toBe('http://100.64.0.7:5912');
	});

	// A tunnel answers on 443, so an explicit scheme is taken at its word.
	it('leaves an explicit scheme alone', () => {
		expect(normalizeConnection('https://enframe.example')).toBe('https://enframe.example');
		expect(normalizeConnection('http://127.0.0.1:5911')).toBe('http://127.0.0.1:5911');
	});

	it('drops trailing slashes, paths and surrounding space', () => {
		expect(normalizeConnection('  192.168.1.9:5911/  ')).toBe('http://192.168.1.9:5911');
		expect(normalizeConnection('http://192.168.1.9:5911/library')).toBe('http://192.168.1.9:5911');
	});

	it('treats an empty address as no connection', () => {
		expect(normalizeConnection('   ')).toBe('');
	});
});

// This is the branch every path-shaped helper reads: get it wrong in the permissive
// direction and a remotely-connected shell hands the server paths from the wrong machine.
describe('isLocalInstance', () => {
	it('recognises the instance on this machine', () => {
		expect(isLocalInstance('http://127.0.0.1:5911')).toBe(true);
		expect(isLocalInstance('http://localhost:5911')).toBe(true);
		expect(isLocalInstance('http://[::1]:5911')).toBe(true);
	});

	it('treats every other instance as another machine', () => {
		expect(isLocalInstance('http://100.64.0.7:5911')).toBe(false);
		expect(isLocalInstance('http://192.168.1.9:5911')).toBe(false);
		expect(isLocalInstance('https://enframe.example')).toBe(false);
		// A hostname that merely starts with the loopback address is a different host.
		expect(isLocalInstance('http://127.0.0.1.example.com:5911')).toBe(false);
	});

	// A browser is served by its instance and shares no disk with it either way.
	it('treats the same-origin browser client as remote', () => {
		expect(isLocalInstance('')).toBe(false);
	});
});
