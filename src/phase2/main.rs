use hound;

// FFI bindings for Nuked-OPM
// The opm_t structure is 1396 bytes
#[repr(C)]
#[repr(align(8))]
struct OpmChip {
    _data: [u8; 1400], // Use actual size with some padding
}

#[link(name = "opm", kind = "static")]
extern "C" {
    fn OPM_Clock(chip: *mut OpmChip, output: *mut i32, sh1: *mut u8, sh2: *mut u8, so: *mut u8);
    fn OPM_Write(chip: *mut OpmChip, port: u32, data: u8);
    fn OPM_Reset(chip: *mut OpmChip);
    fn OPM_SetIC(chip: *mut OpmChip, ic: u8);
}

// Sample rate for WAV output
const SAMPLE_RATE: u32 = 44100;
// Number of chip clock cycles per audio sample
const CLOCKS_PER_SAMPLE: usize = 64;
// Number of clock cycles between register writes (for chip processing)
const CLOCKS_BETWEEN_WRITES: usize = 10;
// Calculate the number of chip cycles for 10ms
// At 64 clocks per sample and 44100 Hz: 10ms = 0.01s * 44100 samples/s * 64 clocks/sample
const CYCLES_10MS: usize = ((SAMPLE_RATE as f64 * 0.01) * CLOCKS_PER_SAMPLE as f64) as usize;

struct Ym2151 {
    chip: Box<OpmChip>,
}

impl Ym2151 {
    fn new() -> Self {
        let mut chip = Box::new(OpmChip {
            _data: [0; 1400],
        });
        
        unsafe {
            OPM_Reset(chip.as_mut());
            OPM_SetIC(chip.as_mut(), 0); // Clear IC flag
            
            // Stabilize chip by clocking it
            let mut output = [0i32; 2];
            let mut sh1 = 0u8;
            let mut sh2 = 0u8;
            let mut so = 0u8;
            for _ in 0..100 {
                OPM_Clock(
                    chip.as_mut(),
                    output.as_mut_ptr(),
                    &mut sh1,
                    &mut sh2,
                    &mut so,
                );
            }
        }
        
        Self { chip }
    }
    
    fn write_register(&mut self, address: u8, data: u8) {
        unsafe {
            let mut output = [0i32; 2];
            let mut sh1 = 0u8;
            let mut sh2 = 0u8;
            let mut so = 0u8;
            
            // Write address
            OPM_Write(self.chip.as_mut(), 0, address);
            
            // Clock for address latch
            for _ in 0..CLOCKS_BETWEEN_WRITES {
                OPM_Clock(
                    self.chip.as_mut(),
                    output.as_mut_ptr(),
                    &mut sh1,
                    &mut sh2,
                    &mut so,
                );
            }
            
            // Write data
            OPM_Write(self.chip.as_mut(), 1, data);
            
            // Clock for data write
            for _ in 0..CLOCKS_BETWEEN_WRITES {
                OPM_Clock(
                    self.chip.as_mut(),
                    output.as_mut_ptr(),
                    &mut sh1,
                    &mut sh2,
                    &mut so,
                );
            }
        }
    }
    
    fn generate_sample(&mut self) -> (i16, i16) {
        let mut output = [0i32; 2];
        let mut sh1 = 0u8;
        let mut sh2 = 0u8;
        let mut so = 0u8;
        
        unsafe {
            for _ in 0..CLOCKS_PER_SAMPLE {
                OPM_Clock(
                    self.chip.as_mut(),
                    output.as_mut_ptr(),
                    &mut sh1,
                    &mut sh2,
                    &mut so,
                );
            }
        }
        
        // Shift right by 5 bits to reduce amplitude (as in the example)
        // and convert to 16-bit samples
        let left = (output[0] >> 5).clamp(-32768, 32767) as i16;
        let right = (output[1] >> 5).clamp(-32768, 32767) as i16;
        
        (left, right)
    }
    
    fn generate_samples(&mut self, count: usize) -> Vec<(i16, i16)> {
        let mut samples = Vec::with_capacity(count);
        for _ in 0..count {
            samples.push(self.generate_sample());
        }
        samples
    }
    
    fn write_with_delay(&mut self, address: u8, data: u8, samples: &mut Vec<(i16, i16)>) {
        self.write_register(address, data);
        // Consume 10ms worth of samples after register write
        let samples_for_10ms = ((SAMPLE_RATE as f64) * 0.01) as usize;
        let new_samples = self.generate_samples(samples_for_10ms);
        samples.extend(new_samples);
    }
}

fn setup_440hz_tone(ym: &mut Ym2151, samples: &mut Vec<(i16, i16)>) {
    // Reset all channels first
    for ch in 0..8 {
        ym.write_register(0x08, ch);
    }
    
    let channel = 0u8;
    
    // RL_FB_CONNECT: RL=11 (both L/R), FB=0, CON=7
    // 0xC7 = 11000111 binary
    ym.write_with_delay(0x20 + channel, 0xC7, samples);
    
    // KC (Key Code) for A4 (440Hz)
    // 0x4A gives approximately 440Hz
    ym.write_with_delay(0x28 + channel, 0x4A, samples);
    
    // KF (Key Fraction)
    ym.write_with_delay(0x30 + channel, 0x00, samples);
    
    // PMS/AMS
    ym.write_with_delay(0x38 + channel, 0x00, samples);
    
    // Configure all 4 operators
    for op in 0..4 {
        let slot = channel + (op * 8);
        
        // DT1/MUL: MUL=1 for fundamental frequency
        ym.write_with_delay(0x40 + slot, 0x01, samples);
        
        // TL (Total Level): 0 = max volume for operator 0 (carrier), silent for others
        if op == 0 {
            ym.write_with_delay(0x60 + slot, 0x00, samples); // Max volume for carrier
        } else {
            ym.write_with_delay(0x60 + slot, 0x7F, samples); // Silent for modulators
        }
        
        // KS/AR: AR=31 for instant attack
        ym.write_with_delay(0x80 + slot, 0x1F, samples);
        
        // AMS/D1R: D1R=5
        ym.write_with_delay(0xA0 + slot, 0x05, samples);
        
        // DT2/D2R: D2R=5
        ym.write_with_delay(0xC0 + slot, 0x05, samples);
        
        // D1L/RR: D1L=15, RR=7
        ym.write_with_delay(0xE0 + slot, 0xF7, samples);
    }
    
    // Key on: 0x78 | channel
    ym.write_with_delay(0x08, 0x78 | channel, samples);
}

fn main() {
    println!("Generating 440Hz 3-second WAV file using Nuked-OPM...");
    
    let mut ym = Ym2151::new();
    let mut samples = Vec::new();
    
    // Setup the tone
    setup_440hz_tone(&mut ym, &mut samples);
    
    println!("Setup complete, generated {} samples during setup", samples.len());
    
    // Check if we have any non-zero samples
    let non_zero_setup = samples.iter().filter(|(l, r)| *l != 0 || *r != 0).count();
    println!("Non-zero samples in setup: {}", non_zero_setup);
    
    // Calculate total samples needed for 3 seconds
    let total_samples = (SAMPLE_RATE * 3) as usize;
    let remaining_samples = total_samples.saturating_sub(samples.len());
    
    // Generate remaining samples
    if remaining_samples > 0 {
        let new_samples = ym.generate_samples(remaining_samples);
        let non_zero_new = new_samples.iter().filter(|(l, r)| *l != 0 || *r != 0).count();
        println!("Generated {} additional samples, {} non-zero", new_samples.len(), non_zero_new);
        samples.extend(new_samples);
    }
    
    // Truncate to exactly 3 seconds if we have too many samples
    samples.truncate(total_samples as usize);
    
    // Check sample statistics
    if samples.len() > 0 {
        let max_left = samples.iter().map(|(l, _)| l.abs()).max().unwrap_or(0);
        let max_right = samples.iter().map(|(_, r)| r.abs()).max().unwrap_or(0);
        println!("Max amplitude - Left: {}, Right: {}", max_left, max_right);
    }
    
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
    println!();
    println!("To play the file on Windows: start {}", output_path);
}
