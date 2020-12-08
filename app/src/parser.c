/*******************************************************************************
 *   (c) 2019 Zondax GmbH
 *
 *  Licensed under the Apache License, Version 2.0 (the "License");
 *  you may not use this file except in compliance with the License.
 *  You may obtain a copy of the License at
 *
 *      http://www.apache.org/licenses/LICENSE-2.0
 *
 *  Unless required by applicable law or agreed to in writing, software
 *  distributed under the License is distributed on an "AS IS" BASIS,
 *  WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *  See the License for the specific language governing permissions and
 *  limitations under the License.
 ********************************************************************************/

#include "parser.h"

#include <stdio.h>
#include <zxmacros.h>

#include "coin.h"
#include "parser_txdef.h"
#include "rslib.h"
#include "zbuffer.h"
#include "nvdata.h"
#include "zxformat.h"
#include "bech32.h"

#if defined(TARGET_NANOX)
// For some reason NanoX requires this function
void __assert_fail(const char *assertion, const char *file, unsigned int line,
                   const char *function) {
    while (1) {
    };
}
#endif

parser_tx_t parser_state;

typedef struct {
    uint8_t type;
    uint8_t index;
} parser_sapling_t;

parser_error_t parser_parse(parser_context_t *ctx, const uint8_t *data,
                            size_t dataLen) {
    parser_state.state = NULL;
    parser_state.len = 0;

    // TODO
    // CHECK_PARSER_ERR(_parser_init(ctx, data, dataLen, &parser_state.len))

    if (parser_state.len == 0) {
        return parser_context_unexpected_size;
    }

    if (zb_allocate(parser_state.len) != zb_no_error ||
        zb_get(&parser_state.state) != zb_no_error) {
        return parser_init_context_empty;
    }

    parser_error_t err = parser_ok; // TODO;
    return err;
}

parser_error_t parser_validate(const parser_context_t *ctx) {
    uint8_t numItems = 0;
    CHECK_PARSER_ERR(parser_getNumItems(ctx, &numItems));

    char tmpKey[30];
    char tmpVal[30];

    for (uint8_t idx = 0; idx < numItems; idx++) {
        uint8_t pageCount = 0;
        CHECK_PARSER_ERR(parser_getItem(ctx, idx, tmpKey, sizeof(tmpKey),
                                        tmpVal, sizeof(tmpVal), 0, &pageCount))
    }

    return parser_ok;
}

parser_error_t parser_sapling_display_value(uint64_t value, char *outVal,
                                                uint16_t outValLen, uint8_t pageIdx,
                                                uint8_t *pageCount){
    char tmpBuffer[100];
    fpuint64_to_str(tmpBuffer, sizeof(tmpBuffer), value, 0);
    pageString(outVal, outValLen, tmpBuffer, pageIdx, pageCount);
    return parser_ok;
}

//fixme: take base58 encoding
parser_error_t parser_sapling_display_address_t(uint8_t *addr, char *outVal,
                                                uint16_t outValLen, uint8_t pageIdx,
                                                uint8_t *pageCount){


    char tmpBuffer[100];
    array_to_hexstr(tmpBuffer, sizeof(tmpBuffer), addr, 26);
    pageString(outVal, outValLen, tmpBuffer, pageIdx, pageCount);
    return parser_ok;
}

parser_error_t parser_sapling_display_address_s(uint8_t *div, uint8_t *pkd, char *outVal,
                                                uint16_t outValLen, uint8_t pageIdx,
                                                uint8_t *pageCount){

    uint8_t address[43];
    MEMCPY(address, div, 11);
    MEMCPY(address + 11, pkd, 32);
    char tmpBuffer[100];
    bech32EncodeFromBytes(tmpBuffer, sizeof(tmpBuffer),
                          BECH32_HRP,
                          address,
                          sizeof(address),
                          1);
    pageString(outVal, outValLen, tmpBuffer, pageIdx, pageCount);
    return parser_ok;
}

parser_error_t parser_sapling_getTypes(const uint16_t displayIdx, parser_sapling_t *prs){
    uint16_t index = displayIdx;

    if (index < t_inlist_len() * 2 && t_inlist_len() > 0){
        prs->type = 0;
        prs->index= index;
        return parser_ok;
    }
    index -= t_inlist_len() * 2;
    if (index < t_outlist_len() * 2 && t_outlist_len() > 0){
        prs->type = 1;
        prs->index= index;
        return parser_ok;
    }
    index -= t_outlist_len() * 2;
    if (index < spendlist_len() * 2 && spendlist_len() > 0){
        prs->type = 2;
        prs->index= index;
        return parser_ok;
    }
    index -= spendlist_len() * 2;
    if (index < outputlist_len() * 3 && outputlist_len() > 0){
        prs->type = 3;
        prs->index= index;
        return parser_ok;
    }
    prs->type = 4;
    return parser_ok;
}

parser_error_t parser_getNumItems(const parser_context_t *ctx,
                                  uint8_t *num_items) {
    *num_items = t_inlist_len()*2 + t_outlist_len()*2+ spendlist_len() *2 + outputlist_len() * 3 + 1;
    return parser_ok;
}

parser_error_t parser_getItem(const parser_context_t *ctx, uint16_t displayIdx,
                              char *outKey, uint16_t outKeyLen, char *outVal,
                              uint16_t outValLen, uint8_t pageIdx,
                              uint8_t *pageCount) {
    MEMZERO(outKey, outKeyLen);
    MEMZERO(outVal, outValLen);
    snprintf(outKey, outKeyLen, "?");
    snprintf(outVal, outValLen, "?");
    *pageCount = 0;

    uint8_t numItems;
    CHECK_PARSER_ERR(parser_getNumItems(ctx, &numItems))
    CHECK_APP_CANARY()

    if (displayIdx < 0 || displayIdx >= numItems) {
        return parser_no_data;
    }

    *pageCount = 1;

    parser_sapling_t prs;
    MEMZERO(&prs, sizeof(parser_sapling_t));
    CHECK_PARSER_ERR(parser_sapling_getTypes(displayIdx, &prs));
    //fixme: make separate functions
    //fixme: take decimals as ZECs?

    switch(prs.type) {
        case 0 :{
            uint8_t itemnum = prs.index / 2;
            t_input_item_t *item = t_inlist_retrieve_item(itemnum);
            uint8_t itemtype = prs.index % 2;
            switch (itemtype) {
                case 0: {
                    snprintf(outKey, outKeyLen, "T-in address");
                    return parser_sapling_display_address_t(item->script, outVal, outValLen, pageIdx, pageCount);
                }
                case 1: {
                    snprintf(outKey, outKeyLen, "T-in ZECs");
                    return parser_sapling_display_value(item->value, outVal, outValLen, pageIdx, pageCount);
                }
            }
        }

        case 1 :{
            uint8_t itemnum = prs.index / 2;
            t_output_item_t *item = t_outlist_retrieve_item(itemnum);
            uint8_t itemtype = prs.index % 2;
            switch (itemtype) {
                case 0: {
                    snprintf(outKey, outKeyLen, "T-out address");
                    return parser_sapling_display_address_t(item->address, outVal, outValLen, pageIdx, pageCount);
                }
                case 1: {
                    snprintf(outKey, outKeyLen, "T-out ZECs");
                    return parser_sapling_display_value(item->value, outVal, outValLen, pageIdx, pageCount);
                }
            }
        }
        case 2: {
            uint8_t itemnum = prs.index / 2;
            spend_item_t *item = spendlist_retrieve_item(itemnum);
            uint8_t itemtype = prs.index % 2;
            switch (itemtype) {
                case 0: {
                    snprintf(outKey, outKeyLen, "S-in address");
                    return parser_sapling_display_address_s(item->div, item->pkd, outVal, outValLen, pageIdx, pageCount);
                }
                case 1: {
                    snprintf(outKey, outKeyLen, "S-in ZECs");
                    return parser_sapling_display_value(item->value, outVal, outValLen, pageIdx, pageCount);
                }
            }
        }

        case 3: {
            uint8_t itemnum = prs.index / 3;
            output_item_t *item = outputlist_retrieve_item(itemnum);
            uint8_t itemtype = prs.index % 3;
            switch (itemtype) {
                case 0: {
                    snprintf(outKey, outKeyLen, "S-out address");
                    return parser_sapling_display_address_s(item->div, item->pkd, outVal, outValLen, pageIdx, pageCount);
                }
                case 1: {
                    snprintf(outKey, outKeyLen, "S-out ZECs");
                    return parser_sapling_display_value(item->value, outVal, outValLen, pageIdx, pageCount);
                }
                case 2: {
                    snprintf(outKey, outKeyLen, "Memotype");
                    if(item->memotype == 0xf6) {
                        snprintf(outVal, outValLen, "Default");
                    }else{
                        snprintf(outVal, outValLen, "Non-default");
                    }
                    return parser_ok;
                }
            }
        }

        case 4: {
            snprintf(outKey, outKeyLen, "Txfee");
            return parser_sapling_display_value(get_valuebalance(), outVal, outValLen, pageIdx, pageCount);
        }

        default: {
            return parser_no_data;
        }
    }
    return parser_ok;
}

void parser_resetState() { zb_deallocate(); }

const char *parser_getErrorDescription(parser_error_t err) {
    switch (err) {
        // General errors
        case parser_ok:
            return "No error";
        case parser_no_data:
            return "No more data";
        case parser_init_context_empty:
            return "Initialized empty context";
        case parser_display_idx_out_of_range:
            return "display_idx_out_of_range";
        case parser_display_page_out_of_range:
            return "display_page_out_of_range";
        case parser_unexepected_error:
            return "Unexepected internal error";
        case parser_no_memory_for_state:
            return "No enough memory for parser state";
            /////////// Context specific
        case parser_context_mismatch:
            return "context prefix is invalid";
        case parser_context_unexpected_size:
            return "context unexpected size";
        case parser_context_invalid_chars:
            return "context invalid chars";
            // Required fields error
            // Coin specific
        case parser_invalid_output_script:
            return "Invalid output script";
        case parser_unexpected_type:
            return "Unexpected data type";
        case parser_unexpected_method:
            return "Unexpected method";
        case parser_unexpected_buffer_end:
            return "Unexpected buffer end";
        case parser_unexpected_value:
            return "Unexpected value";
        case parser_unexpected_number_items:
            return "Unexpected number of items";
        case parser_unexpected_characters:
            return "Unexpected characters";
        case parser_unexpected_field:
            return "Unexpected field";
        case parser_value_out_of_range:
            return "Value out of range";
        case parser_invalid_address:
            return "Invalid address format";
        default:
            return "Unrecognized error code";
    }
}
