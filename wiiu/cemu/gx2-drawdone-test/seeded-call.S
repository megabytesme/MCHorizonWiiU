	.text
	.global draw_done_with_guest_r3
	.type draw_done_with_guest_r3,@function
draw_done_with_guest_r3:
	stwu 1,-16(1)
	mflr 0
	stw 0,20(1)
	bl GX2DrawDone
	lwz 0,20(1)
	mtlr 0
	addi 1,1,16
	blr
	.size draw_done_with_guest_r3,.-draw_done_with_guest_r3
