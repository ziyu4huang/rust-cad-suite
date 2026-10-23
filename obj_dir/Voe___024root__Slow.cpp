// Verilated -*- C++ -*-
// DESCRIPTION: Verilator output: Design implementation internals
// See Voe.h for the primary calling header

#include "Voe__pch.h"

void Voe___024root___ctor_var_reset(Voe___024root* vlSelf);

Voe___024root::Voe___024root(Voe__Syms* symsp, const char* namep)
    : __VdlySched{*symsp->_vm_contextp__}
 {
    vlSymsp = symsp;
    vlNamep = strdup(namep);
    // Reset structure values
    Voe___024root___ctor_var_reset(this);
}

void Voe___024root::__Vconfigure(bool first) {
    (void)first;  // Prevent unused variable warning
}

Voe___024root::~Voe___024root() {
    VL_DO_DANGLING(std::free(const_cast<char*>(vlNamep)), vlNamep);
}
