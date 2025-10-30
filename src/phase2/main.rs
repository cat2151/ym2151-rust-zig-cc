use hound;

// FFI bindings for Nuked-OPM
#[repr(C)]
pub struct OpmChip {
    _opaque: [u8; 0],
}

#[link(name = "opm", kind = "static")]
extern "C" {
    fn OPM_Clock(chip: *mut OpmChip, output: *mut i32, sh1: *mut u8, sh2: *mut u8, so: *mut u8);
    fn OPM_Write(chip: *mut OpmChip, port: u32, data: u8);
    fn OPM_Reset(chip: *mut OpmChip);
}

// YM2151 operates at 3.579545 MHz
const CHIP_CLOCK: f64 = 3_579_545.0;
// Sample rate for WAV output
const SAMPLE_RATE: u32 = 44100;
// Cycles per sample
const CYCLES_PER_SAMPLE: f64 = CHIP_CLOCK / SAMPLE_RATE as f64;

// Calculate the number of chip cycles for 10ms
const CYCLES_10MS: usize = (CHIP_CLOCK * 0.01) as usize;

struct Ym2151 {
    chip: Vec<u8>, // Storage for opm_t
}

impl Ym2151 {
    fn new() -> Self {
        // Allocate memory for the chip structure
        // Based on opm.h, the structure is quite large
        let mut chip = vec![0u8; 4096]; // Allocate enough space
        
        unsafe {
            OPM_Reset(chip.as_mut_ptr() as *mut OpmChip);
        }
        
        Self { chip }
    }
    
    fn write_register(&mut self, address: u8, data: u8) {
        unsafe {
            let chip_ptr = self.chip.as_mut_ptr() as *mut OpmChip;
            // Write address
            OPM_Write(chip_ptr, 0, address);
            // Write data
            OPM_Write(chip_ptr, 1, data);
        }
    }
    
    fn clock(&mut self, cycles: usize) -> Vec<(i16, i16)> {
        let mut samples = Vec::new();
        let mut output = [0i32; 2];
        let mut cycle_counter = 0.0;
        
        for _ in 0..cycles {
            unsafe {
                let chip_ptr = self.chip.as_mut_ptr() as *mut OpmChip;
                OPM_Clock(
                    chip_ptr,
                    output.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                );
            }
            
            cycle_counter += 1.0;
            if cycle_counter >= CYCLES_PER_SAMPLE {
                cycle_counter -= CYCLES_PER_SAMPLE;
                // Convert to 16-bit samples and clamp
                let left = output[0].clamp(-32768, 32767) as i16;
                let right = output[1].clamp(-32768, 32767) as i16;
                samples.push((left, right));
            }
        }
        
        samples
    }
    
    fn write_with_delay(&mut self, address: u8, data: u8, samples: &mut Vec<(i16, i16)>) {
        self.write_register(address, data);
        // Consume 10ms of cycles after register write
        let new_samples = self.clock(CYCLES_10MS);
        samples.extend(new_samples);
    }
}

fn setup_440hz_tone(ym: &mut Ym2151, samples: &mut Vec<(i16, i16)>) {
    // Setup YM2151 for a 440Hz tone on channel 0
    
    // Calculate KC and KF for 440Hz
    // For YM2151, the frequency formula is complex, but for 440Hz:
    // KC (Key Code) determines octave and note
    // KF (Key Fraction) fine-tunes the frequency
    
    // For 440Hz (A4), we use:
    // Octave 4, Note A -> KC = 0x4D (approx)
    // KF can be adjusted for fine tuning
    
    let kc: u8 = 0x4D; // Key code for approximately 440Hz
    let kf: u8 = 0x00; // Key fraction
    
    // Channel 0: Set RL (Left/Right output), FB (Feedback), and Connection (Algorithm)
    // 0x20: RL=11 (both channels), FB=0, CONNECT=7 (algorithm 7 - all operators to output)
    ym.write_with_delay(0x20, 0b11000111, samples);
    
    // Set KC (Key Code) for channel 0
    ym.write_with_delay(0x28, kc, samples);
    
    // Set KF (Key Fraction) for channel 0  
    ym.write_with_delay(0x30, kf << 2, samples);
    
    // Set PMS (Phase Modulation Sensitivity) and AMS (Amplitude Modulation Sensitivity)
    ym.write_with_delay(0x38, 0x00, samples);
    
    // Setup operator parameters for all 4 operators (slots 0, 8, 16, 24 for channel 0)
    for op in [0u8, 8, 16, 24] {
        // DT1 (Detune) and MUL (Multiple)
        // MUL=1 for fundamental frequency
        ym.write_with_delay(0x40 + op, 0x01, samples);
        
        // TL (Total Level) - operator volume
        // Lower value = louder. Use 0x20 for moderate volume
        let tl = if op == 24 { 0x10 } else { 0x7F }; // Only carrier (op 24) is audible
        ym.write_with_delay(0x60 + op, tl, samples);
        
        // KS (Key Scale) and AR (Attack Rate)
        // AR=31 for fast attack
        ym.write_with_delay(0x80 + op, 0x1F, samples);
        
        // AMS-EN and D1R (Decay Rate 1)
        ym.write_with_delay(0xA0 + op, 0x00, samples);
        
        // DT2 and D2R (Decay Rate 2)
        ym.write_with_delay(0xC0 + op, 0x00, samples);
        
        // D1L (Decay 1 Level) and RR (Release Rate)
        ym.write_with_delay(0xE0 + op, 0x0F, samples);
    }
    
    // Key On: Turn on all 4 operators for channel 0
    // 0x08: bit 7-4 = operator on/off, bit 2-0 = channel
    ym.write_with_delay(0x08, 0b01111000, samples);
}

fn main() {
    println!("Generating 440Hz 3-second WAV file using Nuked-OPM...");
    
    let mut ym = Ym2151::new();
    let mut samples = Vec::new();
    
    // Setup the tone
    setup_440hz_tone(&mut ym, &mut samples);
    
    // Calculate total samples needed for 3 seconds
    let total_samples = SAMPLE_RATE * 3;
    let remaining_samples = total_samples.saturating_sub(samples.len() as u32);
    
    // Generate remaining samples
    if remaining_samples > 0 {
        let cycles_needed = (remaining_samples as f64 * CYCLES_PER_SAMPLE) as usize;
        let new_samples = ym.clock(cycles_needed);
        samples.extend(new_samples);
    }
    
    // Truncate to exactly 3 seconds if we have too many samples
    samples.truncate(total_samples as usize);
    
    // Write to WAV file
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate: SAMPLE_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let output_path = "output_440hz.wav";
    let mut writer = hound::WavWriter::create(output_path, spec)
        .expect("Failed to create WAV file");
    
    for (left, right) in samples {
        writer.write_sample(left).expect("Failed to write sample");
        writer.write_sample(right).expect("Failed to write sample");
    }
    
    writer.finalize().expect("Failed to finalize WAV file");
    
    println!("Successfully generated {} with {} samples", output_path, total_samples);
}
