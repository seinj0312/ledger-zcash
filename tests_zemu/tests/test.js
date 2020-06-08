import { expect, test } from "jest";
import Zemu from "@zondax/zemu";
import ZCashApp from "@zondax/ledger-zcash";

import { TX_TESTS } from './unshielded_tx';

const Resolve = require("path").resolve;
const APP_PATH = Resolve("../app/bin/app.elf");
const fs = require('fs');

const APP_SEED = "equip will roof matter pink blind book anxiety banner elbow sun young"
const sim_options = {
    logging: true,
    start_delay: 3000,
    custom: `-s "${APP_SEED}"`
//    ,X11: true
};

jest.setTimeout(20000)

describe('Basic checks', function () {
    test('can start and stop container', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
        } finally {
            await sim.close();
        }
    });

    test('get app version', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
            const app = new ZCashApp(sim.getTransport());
            const version = await app.getVersion();
            expect(version.return_code).toEqual(0x9000);

            console.log(version)
        } finally {
            await sim.close();
        }
    });

    test('get unshielded address', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
            const app = new ZCashApp(sim.getTransport());

            const addr = await app.getAddressAndPubKey("m/44'/133'/5'/0/0", true);
            console.log(addr)
            expect(addr.return_code).toEqual(0x9000);

            const expected_addr_raw = "031f6d238009787c20d5d7becb6b6ad54529fc0a3fd35088e85c2c3966bfec050e";
            const expected_addr = "t1KHG39uhsssPkYcAXkzZ5Bk2w1rnFukZvx";

            const addr_raw = addr.address_raw.toString('hex');
            expect(addr_raw).toEqual(expected_addr_raw);
            expect(addr.address).toEqual(expected_addr);

        } finally {
            await sim.close();
        }
    });

    test('show unshielded address', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
            const app = new ZCashApp(sim.getTransport());

            const addrRequest = app.showAddressAndPubKey("m/44'/133'/5'/0/1", true);
            await Zemu.sleep(1000);
            await sim.clickBoth();

            const addr = await addrRequest;
            console.log(addr)
            expect(addr.return_code).toEqual(0x9000);

            const expected_addr_raw = "026f27818e7426a10773226b3553d0afe50a3697bd02652f1b57d67bf648577d11";
            const expected_addr = "t1PYLcQqpxou9Eak4nroMNGKYoxT4HPdHqJ";

            const addr_raw = addr.address_raw.toString('hex');
            expect(addr_raw).toEqual(expected_addr_raw);
            expect(addr.address).toEqual(expected_addr);

        } finally {
            await sim.close();
        }
    });

    test('get shielded address', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
            const app = new ZCashApp(sim.getTransport());

            const addr = await app.getAddressAndPubKey("m/44'/133'/5'/0/0");
            console.log(addr)
            expect(addr.return_code).toEqual(0x9000);

            const expected_addr_raw = "30fac80e962eb83353ff39d8f4fc255bc3464d0d842a257f849682f4903c11f16ab174aaabe27ff7f60269";
            const expected_addr = "zs1xravsr5k96urx5ll88v0flp9t0p5vngdss4z2luyj6p0fypuz8ck4vt54247yllh7cpxjjcxsv";

            const addr_raw = addr.address_raw.toString('hex');
            expect(addr_raw).toEqual(expected_addr_raw);
            expect(addr.address).toEqual(expected_addr);

        } finally {
            await sim.close();
        }
    });

    test('show shielded address', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
            const app = new ZCashApp(sim.getTransport());

            const addrRequest = app.showAddressAndPubKey("m/44'/133'/5'/0'/1");
            await Zemu.sleep(1000);
            await sim.clickBoth();

            const addr = await addrRequest;
            console.log(addr)
            expect(addr.return_code).toEqual(0x9000);

            const expected_addr_raw = "30fac80e962eb83353ff39d8f4fc255bc3464d0d842a257f849682f4903c11f16ab174aaabe27ff7f60269";
            const expected_addr = "zs1xravsr5k96urx5ll88v0flp9t0p5vngdss4z2luyj6p0fypuz8ck4vt54247yllh7cpxjjcxsv";

            const addr_raw = addr.address_raw.toString('hex');
            expect(addr_raw).toEqual(expected_addr_raw);
            expect(addr.address).toEqual(expected_addr);

        } finally {
            await sim.close();
        }
    });

    test('sign unshielded', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
            const app = new ZCashApp(sim.getTransport());

            // Do not await.. we need to click asynchronously
            const signatureRequest = app.sign("m/44'/133'/5'/0/0", "1234");
            await Zemu.sleep(2000);

            // Click right + double
            await sim.clickRight();
            await sim.clickBoth();

            let signature = await signatureRequest;
            console.log(signature)

            expect(signature.return_code).toEqual(0x9000);
        } finally {
            await sim.close();
        }
    });

    // This test tries to demonstrate
    // the functionality of the unshielded raw transaction
    // parser for an input transaction with 1 input and two outputs
    test('parse raw transaction with 1 input - 2 output', async function () {
        const sim = new Zemu(APP_PATH);
        try {
            await sim.start(sim_options);
            const app = new ZCashApp(sim.getTransport());
            const blob = Buffer.from("010000000107578c9aff7cfd240c36fa1400ee130d540f4c3397d24e8bea50a7f061116a87010000006a473044022011aecead8f48e3b342856a8da2f30c4e05bec5dc147a5bc7b382d01bf68ae5c302204126fd77522ae311a88688bce967532456b08c94322ba182a18fb7786e696c610121027e563beec6765850071067e4fcc7a46d00cbb0d675ef8df1b8d15aaeef91a21fffffffff021cbb0100000000001976a91461aac8b58ac880a45fb06eeedfcf3017679778a988ac32432400000000001976a9144fc16e1766808c0ab090be4376cea9d3a0bbe12988ac00000000", "hex");
            const signatureRequest = app.sign("m/44'/133'/5'/0/0", blob);
            await Zemu.sleep(1000);

            // Click right + double
            await sim.clickBoth();

            let signature = await signatureRequest;
            console.log(signature)

            expect(signature.return_code).toEqual(0x9000);
        } finally {
            await sim.close();
        }
    });

});
