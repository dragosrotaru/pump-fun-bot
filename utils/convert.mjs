import bs58 from 'bs58';

function pubkeyToByteArray(pubkeyString) {
    // Remove any potential whitespace
    pubkeyString = pubkeyString.trim();
    
    // Convert the base58 string to a Buffer
    const bytes = bs58.decode(pubkeyString);
    
    // Convert to array format needed for Rust
    const array = Array.from(bytes);
    
    // Format as a Rust array declaration
    const rustArray = `[
    ${array.join(', ')}
]`;
    
    console.log(rustArray);
    return rustArray;
}

const key = process.argv[2];
pubkeyToByteArray(key);