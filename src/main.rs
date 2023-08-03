#[macro_use]
extern crate packed_struct;
#[macro_use]
extern crate bitflags;
extern crate args;
extern crate getopts;

use args::*;
use getopts::Occur;
use std::{error::Error, str::FromStr};

use packed_struct::prelude::*;

#[repr(u8)]
#[derive(PrimitiveEnum_u8, Clone, Copy, Debug, PartialEq)]
pub enum Instruction {
    NOP,
    LDA,
    STA,
    ADD,
    SUB,
    OUT,
    JMP,
    JC,
    JZ,
    HLT,
    LDI,
    ADI,
    LDR,
    ADR,
}

#[derive(PackedStruct, Copy, Clone, Debug)]
#[packed_struct(bit_numbering = "msb0")]
pub struct RomLayout {
    #[packed_field(bits = "0..=7", ty = "enum")]
    instruction: Instruction,
    #[packed_field(bits = "8..=63", endian = "msb")]
    value: Integer<i64, packed_bits::Bits<56>>,
}

impl RomLayout {
    pub fn new(instruction: Instruction, value: i64) -> RomLayout {
        let value = value.into();
        RomLayout { instruction, value }
    }
}

impl FromStr for RomLayout {
    type Err = ();
    fn from_str(input: &str) -> Result<RomLayout, Self::Err> {
        let mut input = input.split(' ').rev().collect::<Vec<&str>>();
        let instruction = input.pop().ok_or(())?;

        let mut value = || -> Result<i64, Self::Err> {
            let s = input.pop().ok_or(())?;
            s.parse::<i64>().or(Err(()))
        };

        match instruction {
            "NOP" => Ok(RomLayout::new(Instruction::NOP, 0)),
            "LDA" => Ok(RomLayout::new(Instruction::LDA, value()?)),
            "STA" => Ok(RomLayout::new(Instruction::STA, value()?)),
            "ADD" => Ok(RomLayout::new(Instruction::ADD, value()?)),
            "SUB" => Ok(RomLayout::new(Instruction::SUB, value()?)),
            "OUT" => Ok(RomLayout::new(Instruction::OUT, 0)),
            "JMP" => Ok(RomLayout::new(Instruction::JMP, value()?)),
            "JC" => Ok(RomLayout::new(Instruction::JC, value()?)),
            "JZ" => Ok(RomLayout::new(Instruction::JZ, value()?)),
            "HLT" => Ok(RomLayout::new(Instruction::HLT, 0)),
            "LDI" => Ok(RomLayout::new(Instruction::LDI, value()?)),
            "ADI" => Ok(RomLayout::new(Instruction::ADI, value()?)),
            "LDR" => Ok(RomLayout::new(Instruction::LDR, value()?)),
            "ADR" => Ok(RomLayout::new(Instruction::ADR, value()?)),
            _ => todo!(),
        }
    }
}

const ROM_SIZE: usize = 256;

type ROM = [RomLayout; ROM_SIZE];
type RAM = [i64; 256];

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    struct ProgramFlags: u64 {
        const NONE  = 0x00000000;
        const CARRY = 0x00000001;
        const ZERO  = 0x00000002;
        const JUMP  = 0x00000010;
    }
}

struct ProgramState {
    program_counter: u8, // same size as the ROM
    ram: RAM,
    rom: ROM,
    reg_a: i64,
    reg_jump: u8,
    flags: ProgramFlags,
}

impl ProgramState {
    fn new(rom: ROM) -> ProgramState {
        ProgramState {
            program_counter: 0,
            ram: [0; 256],
            rom,
            reg_a: 0,
            reg_jump: 0,
            flags: ProgramFlags::NONE,
        }
    }

    fn step(&mut self) -> Instruction {
        let rom = self.rom[self.program_counter as usize];
        let rom_value_index = rom.value.to_primitive() as usize;

        match rom.instruction {
            Instruction::NOP => {}
            Instruction::LDA => self.reg_a = self.ram[rom_value_index],
            Instruction::STA => self.ram[rom_value_index] = self.reg_a,
            Instruction::ADD => self.alu(self.ram[rom_value_index]),
            Instruction::SUB => todo!(),
            Instruction::OUT => println!("{}", self.reg_a),
            Instruction::JMP => {
                self.flags.insert(ProgramFlags::JUMP);
                self.reg_jump = rom_value_index.try_into().unwrap();
            }
            Instruction::JC => {
                if self.flags.contains(ProgramFlags::CARRY) {
                    self.flags.insert(ProgramFlags::JUMP);
                    self.reg_jump = rom_value_index.try_into().unwrap();
                }
            }
            Instruction::JZ => {
                if self.flags.contains(ProgramFlags::ZERO) {
                    self.flags.insert(ProgramFlags::JUMP);
                    self.reg_jump = rom_value_index.try_into().unwrap();
                }
            }
            Instruction::HLT => {}
            Instruction::LDI => self.reg_a = rom.value.into(),
            Instruction::ADI => todo!(),
            Instruction::LDR => self.reg_a = self.rom[rom_value_index].value.into(),
            Instruction::ADR => todo!(),
        }

        self.count();
        rom.instruction
    }

    fn alu(&mut self, value: i64) {
        let (value, carry) = self.reg_a.overflowing_add(value);
        self.flags.set(ProgramFlags::CARRY, carry);
        self.flags.set(ProgramFlags::ZERO, value == 0);
        self.reg_a = value;
    }

    fn count(&mut self) {
        if self.flags.contains(ProgramFlags::JUMP) {
            self.program_counter = self.reg_jump;
            self.flags.remove(ProgramFlags::JUMP);
        } else {
            self.program_counter += 1;
        }
    }
}

fn compile_rom(program: &str) -> Vec<RomLayout> {
    let rom: Result<Vec<_>, ()> = program
        .trim()
        .lines()
        .map(|s| RomLayout::from_str(s))
        .into_iter()
        .collect();
    rom.unwrap()
}

fn read_rom(bytes: &[u8]) -> ROM {
    let mut rom = Vec::new();
    for chunk in bytes.chunks(8) {
        rom.push(RomLayout::unpack_from_slice(chunk).unwrap());
    }

    rom.resize(ROM_SIZE, RomLayout::new(Instruction::HLT, 0));
    rom.try_into().unwrap()
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = Args::new("Emulator 64Bit (Rust)", "It isn't that exciting.");
    args.option(
        "c",
        "compile",
        "Used to compile a ebr file into bytecode.",
        "FILE",
        Occur::Optional,
        None,
    );
    args.option(
        "o",
        "output",
        "Used to name the outputted ebrc file from -c.",
        "FILE",
        Occur::Optional,
        None,
    );
    args.option(
        "r",
        "run",
        "Used to run a ebrc file",
        "FILE",
        Occur::Optional,
        None,
    );

    args.parse(std::env::args().collect::<Vec<_>>())?;

    let source = args.value_of::<String>("compile");
    let output = args.value_of::<String>("output");
    let rom = args.value_of::<String>("run");

    if let Ok(source) = source {
        let source = std::fs::read_to_string(source)?;
        let rom = compile_rom(source.as_str());
        let rom: Vec<u8> = rom.iter().map(|r| r.pack().unwrap()).flatten().collect();

        let output = match output {
            Ok(output) => output,
            Err(_) => "a.ebrc".to_string(),
        };

        std::fs::write(output, rom)?;
    }

    if let Ok(run) = rom {
        let rom = std::fs::read(run)?;
        let mut state = ProgramState::new(read_rom(&rom));
        loop {
            if state.step() == Instruction::HLT {
                break;
            }
        }
    }

    Ok(())
}
