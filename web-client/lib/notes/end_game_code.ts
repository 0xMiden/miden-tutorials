const endGameNoteCode = `
use.miden::note
use.external_contract::game_contract

const.ERR_WRONG_NUMBER_OF_INPUTS = "Note expects exactly 1 note input"

#! Inputs (arguments):  [player_slot]
begin
    push.0 exec.note::get_inputs
    # [num_inputs, inputs_ptr]
    
    eq.1 assert.err=ERR_WRONG_NUMBER_OF_INPUTS
    # [inputs_ptr]
    
    padw movup.4 mem_loadw drop drop drop
    # [player_slot]

    call.game_contract::end_game
end

`;

export default endGameNoteCode;
