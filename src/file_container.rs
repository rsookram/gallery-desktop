use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
};

pub struct FileContainer {
    f: File,
    end_offsets: Vec<u64>,
}

impl FileContainer {
    pub fn open(p: &Path) -> Self {
        let mut f = File::open(p).unwrap();

        let mut buf = [0u8; 8];
        f.read_exact(&mut buf).unwrap();

        assert_eq!(&buf[..4], b"ofc\0");

        let num_files = u32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]);
        let mut end_offsets = vec![0; usize::try_from(num_files).unwrap()];

        let mut offsets_buf = vec![0; end_offsets.len() * 8];
        f.read_exact(&mut offsets_buf).unwrap();

        for (i, chunk) in offsets_buf.chunks_exact(8).enumerate() {
            end_offsets[i] = u64::from_le_bytes([
                chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
            ]);
        }

        Self { f, end_offsets }
    }

    pub fn read_at(&mut self, i: usize) -> Vec<u8> {
        assert!(i < self.len());

        if i == 0 {
            self.f
                .seek(std::io::SeekFrom::Start(
                    8 + 8 * u64::try_from(self.len()).unwrap(),
                ))
                .unwrap();

            let mut buf = vec![0u8; usize::try_from(self.end_offsets[0]).unwrap()];
            self.f.read_exact(&mut buf).unwrap();

            return buf;
        }

        let prev_offset = self.end_offsets[i - 1];
        self.f
            .seek(std::io::SeekFrom::Start(
                8 + 8 * u64::try_from(self.len()).unwrap() + prev_offset,
            ))
            .unwrap();

        let mut buf = vec![0u8; usize::try_from(self.end_offsets[i] - prev_offset).unwrap()];
        self.f.read_exact(&mut buf).unwrap();

        buf
    }

    pub fn len(&self) -> usize {
        self.end_offsets.len()
    }
}
