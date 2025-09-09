const makeMoveNoteCode = `
use.miden::note
use.external_contract::game_contract

const.ERR_WRONG_NUMBER_OF_INPUTS = "Note expects exactly 1 note input"

#! Inputs (arguments):  [field_index]
begin
    push.0 exec.note::get_inputs
    # [num_inputs, inputs_ptr]
    
    eq.1 assert.err=ERR_WRONG_NUMBER_OF_INPUTS
    # [inputs_ptr]
    
    padw movup.4 mem_loadw drop drop drop
    # [field_index]

    call.game_contract::make_a_move
end

`;

export default makeMoveNoteCode;
