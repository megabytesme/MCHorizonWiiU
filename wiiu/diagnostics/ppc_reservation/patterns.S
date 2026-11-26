.text
.align 2
.global preserve_store_success
.type preserve_store_success,@function
preserve_store_success:
    li 5,4
1:
    lwarx 6,3,5
    stwcx. 4,3,5
    bne- 1b
    lwz 3,0(3)
    blr
.size preserve_store_success,.-preserve_store_success

.global preserve_store_mismatch
.type preserve_store_mismatch,@function
preserve_store_mismatch:
    li 5,128
    lwarx 6,0,3
    stwcx. 4,3,5
    bne 1f
    li 3,-1
    blr
1:
    lwz 3,0(3)
    blr
.size preserve_store_mismatch,.-preserve_store_mismatch

.global preserve_mirrored_load
.type preserve_mirrored_load,@function
preserve_mirrored_load:
    srwi 5,3,1
    lwarx 6,5,5
    mr 3,5
    blr
.size preserve_mirrored_load,.-preserve_mirrored_load

.global preserve_mirrored_store
.type preserve_mirrored_store,@function
preserve_mirrored_store:
    srwi 5,3,1
1:
    lwarx 6,0,3
    stwcx. 4,5,5
    bne- 1b
    mr 3,5
    blr
.size preserve_mirrored_store,.-preserve_mirrored_store

.section .note.GNU-stack,"",@progbits
