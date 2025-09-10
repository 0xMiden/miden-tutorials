export const ticTacToeCode = `

use.miden::account
use.miden::note
use.miden::account_id

const.ERR_WRONG_FIELD_INDEX="Wrong field index to make move"
const.ERR_WRONG_PLAYER="Wrong player trying to make move"
const.ERR_FIELD_USED="Field has already been used"
const.ERR_WRONG_PLAYER_SLOT="Wrong player slot supplied"
const.ERR_NO_WINNER="There is no winner or draw"

const.PLAYER1_SLOT=0
const.PLAYER2_SLOT=1
const.FLAG_SLOT=2
const.WINNER_SLOT=3
const.MAPPING_SLOT=4

# => [player1_prefix, player1_suffix, player2_prefix, player2_suffix]
export.constructor
    # store player1 ID by padding value to size of one word
    push.0.0 push.PLAYER1_SLOT
    # [player1_slot, 0, 0, player1_prefix, player1_suffix, player2_prefix, player2_suffix]

    exec.account::set_item
    # [OLD_VALUE, player2_prefix, player2_suffix]

    # drop old value from stack
    dropw
    # [player2_prefix, player2_suffix]

    # pad to word
    push.0.0 push.PLAYER2_SLOT
    # [player2_slot, 0, 0, player2_prefix, player2_suffix]

    # store player2 ID
    exec.account::set_item
    # [OLD_VALUE]

    # Drop old value from stack (returned by set_item call)
    dropw
    # []
end

# => [field_index]
export.make_a_move
    dup push.9 lt assert.err=ERR_WRONG_FIELD_INDEX
    # [field_index]

    exec.note::get_sender
    # [caller_prefix, caller_suffix, field_index]

    # verify caller ID is in line with current player
    push.FLAG_SLOT exec.account::get_item
    # [FLAG, caller_prefix, caller_suffix, field_index]

    # Push zero (to compare to zero)
    padw
    # [0WORD, FLAG, caller_prefix, caller_suffix, field_index]

    # Check if equal
    eqw
    # [is_true, 0WORD, FLAG, caller_prefix, caller_suffix, field_index]

    if.true
        # delete 0WORD & FLAG
        dropw dropw
        # [caller_prefix, caller_suffix, field_index]

        push.PLAYER1_SLOT push.1
        # [1, player1_slot, caller_prefix, caller_suffix, field_index]
        movdn.4
        # [player1_slot, caller_prefix, caller_suffix, field_index, 1]
    else
        # delete 0WORD & FLAG
        dropw dropw
        # [caller_prefix, caller_suffix, field_index]

        push.PLAYER2_SLOT push.2
        # [2, player2_slot, caller_prefix, caller_suffix, field_index]
        movdn.4
        # [player2_slot, caller_prefix, caller_suffix, field_index, 2]
    end

    exec.account::get_item
    # [0, 0, player_prefix, player_suffix, caller_prefix, caller_suffix, field_index, move_value]

    # Remove trailing zero's of word
    drop drop
    # [player_prefix, player_suffix, caller_prefix, caller_suffix, field_index, move_value]

    exec.account_id::is_equal assert.err=ERR_WRONG_PLAYER
    # [field_index, move_value]

    dup dup dup
    # [FIELD_INDEX, move_value]

    dup.4 dup.5 dup.6 movup.7
    # [MOVE_VALUE, FIELD_INDEX]

    swapw
    # [FIELD_INDEX, MOVE_VALUE]

    push.MAPPING_SLOT
    # [mapping_slot, FIELD_INDEX, MOVE_VALUE]

    exec.account::set_map_item
    # [OLD_MAP_ROOT, OLD_MAP_VALUE]

    dropw
    # [OLD_MAP_VALUE]

    padw
    # [EMPTY_WORD, OLD_MAP_VALUE]

    eqw
    # [is_equal, EMPTY_WORD, OLD_MAP_VALUE]

    assert.err=ERR_FIELD_USED
    # [EMPTY_WORD, OLD_MAP_VALUE]

    dropw dropw
    # []

    debug.stack
end

# => [player_slot]
export.end_game
    dup push.3 lt assert.err=ERR_WRONG_PLAYER_SLOT
    # [player_slot]

    # Store all possible winning lines in memory
    push.2.1.0.0 # 0 1 2
    mem_storew.4 dropw

    push.5.4.3.0 # 3 4 5
    mem_storew.8 dropw

    push.8.7.6.0 # 6 7 8
    mem_storew.12 dropw

    push.6.3.0.0 # 0 3 6
    mem_storew.16 dropw

    push.7.4.1.0 # 1 4 7
    mem_storew.20 dropw

    push.8.5.2.0 # 2 5 8
    mem_storew.24 dropw

    push.8.4.0.0 # 0 4 8
    mem_storew.28 dropw

    push.6.4.2.0 # 2 4 6
    mem_storew.32 dropw

    # push i for loop
    push.8
    dup neq.0
    # [true, i, player_slot]

    while.true
        # [i, player_slot]

        swap add.1 swap
        # [i, expected_player_value]

        dup movdn.2 padw
        # [0, 0, 0, 0, i, player_slot, i]

        movup.4 mul.4 mem_loadw
        # [0, index, index, index, expected_player_value, i]

        drop
        # [index, index, index, expected_player_value, i]

        exec.check_line
        # [is_win, expected_player_value, i]

        if.true
            swap drop sub.1
            # [player_slot]

            push.0.0.0 push.WINNER_SLOT
            # [winner_slot, 0, 0, 0, player_slot]

            exec.account::set_item
            # [OLD_VALUE]

            dropw
            # []

            # Push 1 to exit loop
            push.1
            # [1]

            # Push 3 to indicate win
            push.3
            # [3, 1]
        end

        swap
        # [i, player_slot]

        sub.1
        # [i-1, player_slot]

        dup neq.0
        # [true/false, i-1, player_slot]
    end
    # [i-1, player_slot OR 3]

    drop
    # [player_slot OR 3]

    neq.3
    # [not_win]

    if.true
        exec.check_draw
        # [is_draw]

        if.true
            # store 2 at winner_slot

            push.2.0.0.0 push.WINNER_SLOT
            # [winner_slot, 0, 0, 0, 2]

            exec.account::set_item
            # [OLD_VALUE]

            dropw
            # []
        else
            push.0.1
            eq assert.err=ERR_NO_WINNER
        end
    end
end

# []
proc.check_draw
    # check that each item of mapping has equal value except zero (=> win)

    # Add empty zero word (expected value)
    push.0
    # [0]

    # push i for loop
    push.9
    dup neq.0
    # [true, i, 0]

    while.true
        dup
        # [i, i, 0]

        # check field value
        exec.retrieve_board_value
        # [value, i, 0]

        # if not zero => break out of loop and return false
        neq.0
        # [is_zero, i, 0]

        if.true
            swap drop
            # [0]

            add.1
            # [1] <- indicator that not a draw

            # make loop exit by pushing i=1
            push.1
            # [1 (i), 1]

            swap
            # [1, 1 (i)]
        end

        swap
        # [i, 0 OR 1]

        sub.1
        # [i-1, 0 OR 1]

        dup neq.0
        # [true/false, i-1, 0 OR 1]
    end
    # [0, 0 OR 1]

    drop
    # [0 OR 1]

    if.true
        push.0
    else
        push.1
    end
    # [true_or_false]
end

# => [key1, key2, key3, player_value]
proc.check_line
    # add i to stack
    push.3
    # [i, key1, key2, key3, player_value]
    dup neq.0
    # [true, i, key1, key2, key3, player_value]

    while.true
        sub.1
        # [i-1, key_or_value, key_or_value, key_or_value, player_value]

        # Move index to end
        movdn.4
        # [key_or_value, key_or_value, key_or_value, player_value, i-1]

        exec.retrieve_board_value movdn.2
        # [key_or_value, key_or_value, value, player_value, i-1]

        # move index to top
        movup.4
        # [i-1, key_or_value, key_or_value, value, player_value]

        dup neq.0
    end
    # [0, value3, value2, value1, player_value]

    drop
    # [value3, value2, value1, player_value]

    swap.3
    # [player_value, value3, value2, value1]

    dup dup dup dup dup
    # [player_value, PLAYER_VALUE, player_value, value3, value2, value1]

    movdn.8
    # [PLAYER_VALUE, player_value, value3, value2, value1, player_value]

    eqw
    # [is_equal, PLAYER_VALUE, player_value, value3, value2, value1, player_value]

    movdn.8 dropw dropw
    # [is_equal, player_value]
end

# [key]
proc.retrieve_board_value
    dup dup dup

    push.MAPPING_SLOT

    exec.account::get_map_item

    # remove trailing empty fields
    drop drop drop
end
`;

export default ticTacToeCode;
